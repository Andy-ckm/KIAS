<p align="center">
  <img src="docs/logo/kias-logo.svg" alt="KIAS" width="420">
</p>

<h1 align="center">KIAS 功能使用说明书</h1>
<p align="center"><strong>生产级 AI Agent 集群调度框架 — 中文详细文档</strong></p>

---

## 目录

- [1. 快速入门](#1-快速入门)
- [2. Agent 管理](#2-agent-管理)
- [3. Workflow 工作流](#3-workflow-工作流)
- [4. Model Router 模型路由](#4-model-router-模型路由)
- [5. Sandbox 沙箱](#5-sandbox-沙箱)
- [6. MCP Protocol](#6-mcp-protocol)
- [7. 配置文件详解](#7-配置文件详解)
- [8. API 接口说明](#8-api-接口说明)
- [9. 常见问题解答](#9-常见问题解答)

---

## 1. 快速入门

### 1.1 安装

```bash
curl -fsSL https://raw.githubusercontent.com/Andy-ckm/KIAS/main/install.sh | sh
```

或从源码编译：

```bash
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS
make build
```

### 1.2 初始化配置

```bash
kias config init
```

这会在 `~/.kias/config.json` 创建默认配置文件。

### 1.3 配置 LLM 模型

编辑 `config/kias.toml`（服务端配置）：

```toml
[model]
provider = "openai"
api_key = "sk-your-key"
model = "gpt-4o"
```

### 1.4 启动服务

```bash
kias server start
# 或指定配置文件
kias server start --config config/kias.toml
```

启动后可通过 `http://localhost:8080` 访问 Dashboard，`http://localhost:8080/health` 检查服务状态。

### 1.5 全局 CLI 参数

所有 `kias` 命令均支持以下全局参数：

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--output <format>` | 输出格式：`json`、`table`、`yaml`、`quiet` | `json` |
| `--dry-run` | 只验证不执行 | `false` |
| `--namespace <ns>` | 命名空间 | `default` |
| `--server <url>` | API 服务地址（或设置 `KIAS_SERVER` 环境变量） | `http://localhost:8080` |
| `--api-key <key>` | API 密钥（或设置 `KIAS_API_KEY` 环境变量） | 无 |
| `-v, --verbose` | 输出详细日志 | `false` |

---

## 2. Agent 管理

Agent 是 KIAS 的核心执行单元，每个 Agent 绑定一个 LLM 模型、系统 Prompt 和可选的工具/技能集。

### 2.1 Agent 定义文件（YAML）

KIAS 采用 Kubernetes 风格的声明式定义，文件格式如下：

```yaml
# my-agent.yaml
apiVersion: kias/v1
kind: Agent
metadata:
  name: my-assistant
  namespace: default
  labels:
    env: production
    team: backend
  annotations:
    description: "生产环境助手"
spec:
  prompt: "你是一个有帮助的助手，擅长回答技术问题。"
  model:
    name: gpt-4o
    temperature: 0.7        # 可选，范围 0.0-2.0
    max_tokens: 4096        # 可选，默认 4096
  tools:                    # 可选，绑定的工具列表
    - web_search
    - code_exec
  skills:                   # 可选，绑定的技能列表
    - summarization
  sandboxes:                # 可选，关联的沙箱
    - python3.11
  resources:                # 可选，资源限制
    memory: "512Mi"
    cpu: 0.5
    gpu: "1"
  permissions:              # 可选，权限控制
    read: ["/data/*"]
    write: ["/output/*"]
    deny: ["/etc/*"]
  cost:                     # 可选，成本控制
    max_tokens_per_run: 10000
    max_cost_per_day: 5.0
    max_cost_per_run: 0.5
  audit:                    # 可选，审计配置
    log_level: detailed
    retention: 90d
  retry:                    # 可选，重试策略
    max_retries: 3
    backoff_ms: 1000
  timeout: 300              # 可选，超时时间（秒）
```

**字段约束：**
- `metadata.name`：只允许小写字母、数字和连字符（`-`）
- `apiVersion`：必须为 `kias/v1`
- `kind`：必须为 `Agent`
- `spec.prompt`：不能为空
- `spec.model.name`：不能为空
- `temperature`：范围 `0.0` ~ `2.0`
- `max_tokens`：不能为 `0`
- `timeout`：不能为 `0`

### 2.2 创建 Agent

```bash
# 从 YAML 文件创建
kias agent apply --file my-agent.yaml

# Dry-run 模式（只验证不部署）
kias agent apply --file my-agent.yaml --dry-run

# 只渲染运行时配置（本地校验）
kias agent render --file my-agent.yaml
```

**输出示例：**

```
✓: Agent 'my-assistant' 定义验证通过
✓: Agent 'my-assistant' 已成功应用
{
  "id": "agent-a1b2c3d4",
  "name": "my-assistant",
  "status": "Running",
  "model": "gpt-4o",
  "created_at": "2024-06-15T10:30:00Z"
}
```

### 2.3 运行 Agent

```bash
# 交互式运行（带完整输出）
kias agent run --name my-assistant --prompt "请帮我总结 Rust 的特点"

# CI 友好调用（只输出纯文本结果）
kias agent invoke --name my-assistant --text "什么是 KIAS？" --text-only

# 指定超时时间
kias agent invoke --name my-assistant --text "分析这段代码" --timeout 600

# 指定输出格式
kias agent invoke --name my-assistant --text "你好" --output table
```

**输出示例（invoke）：**

```json
{
  "run_id": "run-xyz789",
  "agent_id": "agent-a1b2c3d4",
  "status": "Completed",
  "output": "Rust 是一门系统编程语言，特点包括内存安全、零成本抽象、并发安全等。",
  "usage": {
    "prompt_tokens": 150,
    "completion_tokens": 200,
    "total_tokens": 350
  },
  "latency_ms": 1200
}
```

### 2.4 查看 Agent

```bash
# 列出所有 Agent
kias agent list

# 按标签过滤
kias agent list --label "env=production"

# 查看单个 Agent 详情
kias agent get --name my-assistant

# 查看 Agent 日志
kias agent logs --name my-assistant
kias agent logs --name my-assistant --follow --tail 50

# 查看 Agent 事件
kias agent events --name my-assistant
kias agent events --name my-assistant --event-type Error
```

### 2.5 删除 Agent

```bash
# 需要 --force 确认
kias agent delete --name my-assistant --force
```

### 2.6 更新 Agent 状态

```bash
# 通过 API 更新状态
curl -X PATCH http://localhost:8080/api/v1/agents/{agent_id}/status \
  -H "Authorization: Bearer <api_key>" \
  -H "Content-Type: application/json" \
  -d '{"status": "Paused"}'
```

---

## 3. Workflow 工作流

Workflow 是 DAG（有向无环图）形式的多 Agent 编排引擎。支持 Shell、HTTP、LLM 三种执行器，并支持条件分支、子图和检查点持久化。

### 3.1 Workflow 定义文件

```yaml
# data-pipeline.yaml
apiVersion: kias/v1
kind: Workflow
metadata:
  name: data-pipeline
  namespace: default
spec:
  entry: fetch-data          # 入口节点
  nodes:
    - name: fetch-data
      agent: data-fetcher
      prompt: "从 API 获取原始数据"
    - name: clean-data
      agent: data-cleaner
      prompt: "清洗和标准化数据"
    - name: analyze
      agent: analyzer
      prompt: "分析数据并生成报告"
      condition: "data_quality > 0.8"   # 条件执行
    - name: notify
      agent: notifier
      prompt: "发送完成通知"
  edges:
    - from: fetch-data
      to: clean-data
    - from: clean-data
      to: analyze
      condition: "status == 'success'"
    - from: clean-data
      to: notify
      condition: "status == 'failed'"
    - from: analyze
      to: notify
```

### 3.2 执行器类型

WorkflowEngine 内置四种节点执行器：

| 执行器 | 说明 | 用途 |
|--------|------|------|
| **ShellExecutor** | 在沙箱中执行 Shell 命令 | 数据处理、脚本运行 |
| **HttpExecutor** | 发送 HTTP 请求 | 调用外部 API |
| **LlmExecutor** | 调用 LLM 模型 | 文本生成、分析 |
| **SubWorkflowExecutor** | 嵌套执行子工作流 | 复杂流程复用 |

### 3.3 工作流操作

```bash
# 应用工作流定义
kias workflow apply --file data-pipeline.yaml

# Dry-run 模式
kias workflow apply --file data-pipeline.yaml --dry-run

# 运行工作流
kias workflow run data-pipeline

# 带输入参数运行
kias workflow run data-pipeline --input '{"source": "https://api.example.com/data"}'

# 查看运行状态
kias workflow status run-001

# 查看运行日志
kias workflow logs run-001

# 列出所有工作流
kias workflow list
```

**输出示例（run）：**

```json
{
  "name": "data-pipeline",
  "status": "Running",
  "nodes": [
    {"name": "fetch-data", "status": "Completed", "duration_ms": 3200},
    {"name": "clean-data", "status": "Completed", "duration_ms": 1500},
    {"name": "analyze", "status": "Running", "duration_ms": null}
  ]
}
```

### 3.4 状态检查点

WorkflowEngine 自动在每个节点完成后保存检查点，支持：
- **崩溃恢复**：重启后从最近检查点继续执行
- **重放调试**：查看每一步的输入输出
- **存储后端**：`InMemoryCheckpointStore`（测试）或 `SqliteCheckpointStore`（生产）

---

## 4. Model Router 模型路由

Model Router 是智能多模型路由层，支持云端 API 和本地模型的统一调用，内置负载均衡、熔断器、成本追踪。

### 4.1 支持的模型提供商

#### 云端 API

| 供应商 | 模型 | 环境变量 |
|--------|------|----------|
| **OpenAI** | GPT-4o, GPT-4, GPT-3.5 | `OPENAI_API_KEY` |
| **Anthropic** | Claude 3.5 Sonnet, Claude 3 Opus | `ANTHROPIC_API_KEY` |
| **Google** | Gemini 1.5 Pro/Flash | `GOOGLE_API_KEY` |
| **Azure OpenAI** | 全系 OpenAI 模型 | `AZURE_OPENAI_ENDPOINT` |
| **AWS Bedrock** | Claude, Llama, Mistral | `AWS_ACCESS_KEY_ID` |
| **OpenRouter** | 100+ 模型 | `OPENROUTER_API_KEY` |
| **DeepSeek** | DeepSeek-V2, DeepSeek-Coder | `DEEPSEEK_API_KEY` |
| **Qwen (通义)** | Qwen-Max, Qwen-Plus | `DASHSCOPE_API_KEY` |

#### 本地模型

| 服务 | 地址 | 适用场景 |
|------|------|----------|
| **Ollama** | `http://localhost:11434` | 开发测试 |
| **vLLM** | `http://localhost:8000` | 生产高吞吐 |
| **llama.cpp** | `http://localhost:8080` | 边缘设备 |
| **LocalAI** | `http://localhost:8080` | OpenAI 兼容 |
| **TGI (HuggingFace)** | `http://localhost:8080` | HuggingFace 生态 |

### 4.2 路由策略

| 策略 | 说明 |
|------|------|
| `RoundRobin` | 轮询（默认） |
| `LeastLatency` | 最低延迟优先 |
| `CostOptimized` | 最低成本优先 |
| `CapabilityBased` | 按能力匹配（Vision、LongContext 等） |
| `WeightedRandom` | 加权随机 |
| `Pinned` | 固定到指定供应商 |
| `LeastBusy` | 最少活跃请求优先 |
| `UsageBased` | 按 TPM/RPM 限制路由 |

### 4.3 配置示例

#### 云端模型配置

编辑 `~/.kias/config.toml`：

```toml
[model]
provider = "openai"
api_key = "sk-your-openai-key"
model = "gpt-4o"

# 或 Anthropic
# provider = "anthropic"
# api_key = "sk-ant-your-key"
# model = "claude-3-5-sonnet-20241022"
```

#### 本地模型配置（Ollama）

```toml
[model]
provider = "ollama"
endpoint = "http://localhost:11434"
model = "llama3.1:8b"
```

#### 多模型路由配置（JSON）

```json
{
  "name": "my-router",
  "description": "生产环境多模型路由",
  "tool_type": "Mcp",
  "config": {
    "endpoint": "http://localhost:8080/v1/chat/completions",
    "parameters": {
      "strategy": "LeastLatency",
      "providers": [
        {
          "name": "openai",
          "api_key": "sk-xxx",
          "models": ["gpt-4o", "gpt-4o-mini"],
          "weight": 3
        },
        {
          "name": "anthropic",
          "api_key": "sk-ant-xxx",
          "models": ["claude-3-5-sonnet-20241022"],
          "weight": 2
        },
        {
          "name": "ollama",
          "endpoint": "http://localhost:11434",
          "models": ["llama3.1:8b"],
          "weight": 1
        }
      ]
    }
  }
}
```

### 4.4 CLI 操作

```bash
# 注册模型服务
kias model register --file model-config.json

# 列出已注册模型
kias model list

# 测试模型连通性
kias model test --name gpt-4o --prompt "Hello, world!"
```

### 4.5 模型能力标签

ModelRouter 支持按能力自动路由：

| 能力 | 说明 |
|------|------|
| `Chat` | 对话补全 |
| `Completion` | 文本补全 |
| `Embedding` | 文本嵌入 |
| `ImageGeneration` | 图片生成 |
| `Vision` | 图片理解 |
| `FunctionCalling` | 工具调用 |
| `Streaming` | 流式响应 |
| `LongContext` | 长上下文（>32k） |
| `CodeGeneration` | 代码生成 |
| `Reasoning` | 推理/思考 |

### 4.6 本地模型参数调优

通过 JSON 配置或代码设置：

```json
{
  "model_params": {
    "temperature": 0.7,
    "top_p": 0.9,
    "top_k": 40,
    "max_tokens": 4096,
    "repeat_penalty": 1.1,
    "stop": ["\n\n"],
    "system_prompt": "你是一个有帮助的助手。"
  }
}
```

---

## 5. Sandbox 沙箱

Sandbox 为 Agent 提供安全隔离的执行环境。支持多种隔离级别和后端。

### 5.1 隔离级别

| 后端 | 隔离级别 | 说明 |
|------|----------|------|
| **Process** | 进程级 | 轻量级，通过 cgroup/namespace 隔离 |
| **Docker** | 容器级 | 完整容器隔离，适合生产 |
| **gVisor** | 内核级 | 用户态内核，更强隔离 |
| **Firecracker** | microVM | 虚拟机级别隔离 |
| **Wasm** | 沙箱 | WebAssembly 沙箱 |

### 5.2 预置模板

| 模板名 | 镜像 | 说明 |
|--------|------|------|
| `python3.11` | `python:3.11-slim` | Python 3.11 环境 |
| `node20` | `node:20-slim` | Node.js 20 环境 |
| `rust1.75` | `rust:1.75-slim` | Rust 编译环境 |
| `ubuntu22.04` | `ubuntu:22.04` | 通用 Ubuntu 环境 |

### 5.3 CLI 操作

```bash
# 创建沙箱
kias sandbox create --template python3.11
kias sandbox create --template node20 --name my-sandbox

# 在沙箱中执行命令
kias sandbox exec <sandbox-id> -- python3 -c "print('hello')"
kias sandbox exec <sandbox-id> -- ls -la /workspace

# 列出所有沙箱
kias sandbox list

# 销毁沙箱
kias sandbox destroy <sandbox-id>
```

**输出示例（create）：**

```
→: 创建沙箱 'sandbox-a1b2c3d4' (模板: python3.11)
{
  "id": "sandbox-a1b2c3d4",
  "name": "my-sandbox",
  "template": "python3.11",
  "status": "Running",
  "created_at": "2024-06-15T10:30:00Z"
}
```

### 5.4 沙箱资源配置

```json
{
  "name": "python3.11",
  "description": "Python 3.11 environment",
  "image": "python:3.11-slim",
  "resources": {
    "memory": "512Mi",
    "cpu": 0.5,
    "disk": "1Gi"
  }
}
```

### 5.5 沙箱状态

| 状态 | 说明 |
|------|------|
| `Creating` | 创建中 |
| `Running` | 运行中 |
| `Stopped` | 已停止 |
| `Error` | 异常 |

---

## 6. MCP Protocol

MCP（Model Context Protocol）是 KIAS 的工具注册和调用协议，基于 JSON-RPC 2.0，支持多种传输层和认证方式。

### 6.1 支持的传输层

| 传输方式 | 说明 |
|----------|------|
| **stdio** | 标准输入/输出（本地进程） |
| **HTTP + SSE** | HTTP 服务 + Server-Sent Events |
| **In-Memory** | 内存传输（测试用） |

### 6.2 工具类型

| 类型 | 说明 |
|------|------|
| `Mcp` | MCP 协议工具 |
| `FunctionCall` | 函数调用 |
| `Http` | HTTP API 调用 |
| `Shell` | Shell 命令执行 |

### 6.3 注册工具

创建工具定义文件 `my-tool.json`：

```json
{
  "name": "web_search",
  "description": "搜索互联网获取最新信息",
  "tool_type": "Mcp",
  "config": {
    "endpoint": "http://localhost:3000",
    "command": null,
    "parameters": {
      "max_results": 10,
      "language": "zh-CN"
    }
  }
}
```

```bash
# 注册工具
kias tool register --file my-tool.json

# 列出工具
kias tool list

# 测试工具
kias tool test web_search --input '{"query": "KIAS 框架"}'
```

### 6.4 注册技能

创建技能定义文件 `my-skill.json`：

```json
{
  "name": "summarization",
  "description": "文本摘要技能",
  "version": "1.0.0",
  "tags": ["nlp", "text", "summary"],
  "parameters": {
    "max_length": 500,
    "style": "concise"
  }
}
```

```bash
# 注册技能
kias skill register --file my-skill.json

# 列出技能
kias skill list

# 搜索技能
kias skill search "文本处理"
```

### 6.5 MCP 高级特性

通过 Cargo feature 门控启用：

| Feature | 说明 |
|---------|------|
| `auth` | OAuth 2.0、API Key 认证、RBAC |
| `resilience` | 熔断器、速率限制 |
| `metrics` | 指标收集、Prometheus 导出 |
| `credentials` | 凭证管理、加密存储、轮换策略 |
| `hot-reload` | 从 YAML/JSON 文件热加载工具定义 |
| `sandbox` | 沙箱执行环境 |
| `docker` | Docker 沙箱后端 |

---

## 7. 配置文件详解

KIAS 有两层配置：**服务端配置**（`config/kias.toml`）和 **CLI 配置**（`~/.kias/config.json`）。

### 7.1 服务端配置（config/kias.toml）

配置加载优先级：`config/default.toml` → `KIAS_CONFIG` 环境变量指定的文件 → `KIAS_` 前缀的环境变量覆盖。

```toml
# ── 日志配置 ────────────────────────────────────
[logging]
level = "info"       # trace, debug, info, warn, error
format = "text"      # text 或 json

# ── API 服务器 ──────────────────────────────────
[api_server]
host = "0.0.0.0"            # 绑定地址
port = 8080                  # 绑定端口
tls = false                  # 是否启用 TLS
tls_cert_path = "/path/to/cert.pem"     # TLS 证书
tls_key_path = "/path/to/key.pem"       # TLS 私钥
tls_client_ca_path = "/path/to/ca.pem"  # mTLS CA 证书
tls_min_version = "1.3"     # 最低 TLS 版本：1.2 或 1.3
auth_enabled = false         # 是否启用 API Key 认证
api_keys = ["sk-key1", "sk-key2"]  # 有效 API Key 列表
jwt_secret = "your-secret"   # JWT 密钥（可选）
jwt_issuer = "kias"          # JWT 签发者（可选）
jwt_expiration_hours = 24    # JWT 过期时间（小时）

# ── 调度器 ──────────────────────────────────────
[scheduler]
algorithm = "cache_aware"    # round_robin, least_loaded, resource_aware, cache_aware
interval_ms = 1000           # 调度间隔（毫秒）

# ── 控制器 ──────────────────────────────────────
[controller]
heartbeat_interval_secs = 15   # 心跳间隔（秒）
failure_timeout_secs = 60      # 故障检测超时（秒）
max_retries = 3                # 最大重试次数

# ── 可观测性 ────────────────────────────────────
[agentsight]
enabled = true               # 是否启用
metrics_port = 9090          # Prometheus 指标端口

# ── 缓存 ────────────────────────────────────────
[cache_hub]
enabled = true
max_entries = 10000          # 最大缓存条目
ttl_secs = 3600              # TTL（秒）

# ── 知识库 ──────────────────────────────────────
[knowledge]
enabled = false
embedding_model = "text-embedding-ada-002"  # 嵌入模型

# ── 存储 ────────────────────────────────────────
[storage]
etcd_endpoints = "http://localhost:2379"
sqlite_url = "sqlite://kias.db"    # SQLite 数据库路径
cache_mode = "sqlite"              # sqlite（持久化）或 memory（内存）
```

#### 环境变量覆盖

所有配置项均可通过 `KIAS_` 前缀的环境变量覆盖，层级用 `__` 分隔：

```bash
# 覆盖 api_server.port
export KIAS_API_SERVER__PORT=9090

# 覆盖 logging.level
export KIAS_LOGGING__LEVEL=debug

# 覆盖 storage.sqlite_url
export KIAS_STORAGE__SQLITE_URL="sqlite:///data/kias.db"
```

### 7.2 CLI 配置（~/.kias/config.json）

```json
{
  "profiles": [
    {
      "name": "default",
      "api_endpoint": "http://localhost:8080",
      "api_key": null,
      "namespace": "default",
      "output_format": "json"
    },
    {
      "name": "production",
      "api_endpoint": "https://api.kias.io",
      "api_key": "sk-prod-key",
      "namespace": "production",
      "output_format": "table"
    }
  ],
  "active_profile": "default"
}
```

#### CLI 配置操作

```bash
# 初始化配置
kias config init

# 设置配置项
kias config set server http://prod-server:8080
kias config set api_key sk-your-key
kias config set namespace production
kias config set output table

# 获取配置项
kias config get server
kias config get api_key
kias config get active_profile

# 列出所有配置
kias config list
```

---

## 8. API 接口说明

KIAS API Server 基于 Axum 框架，提供 RESTful API 和 WebSocket 实时通信。

### 8.1 健康检查（无需认证）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 存活探针 |
| GET | `/readyz` | 就绪探针 |
| GET | `/healthz/deep` | 深度健康检查（含依赖） |

```bash
curl http://localhost:8080/health
# 200 OK
```

### 8.2 Agent API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/agents` | 列出所有 Agent |
| POST | `/api/v1/agents` | 创建 Agent |
| GET | `/api/v1/agents/:id` | 获取 Agent 详情 |
| DELETE | `/api/v1/agents/:id` | 删除 Agent |
| POST | `/api/v1/agents/:id/invoke` | 调用 Agent |
| PATCH | `/api/v1/agents/:id/status` | 更新 Agent 状态 |

```bash
# 创建 Agent
curl -X POST http://localhost:8080/api/v1/agents \
  -H "Authorization: Bearer <api_key>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-agent",
    "prompt": "你是一个助手",
    "model": "gpt-4o",
    "temperature": 0.7
  }'

# 调用 Agent
curl -X POST http://localhost:8080/api/v1/agents/{id}/invoke \
  -H "Authorization: Bearer <api_key>" \
  -H "Content-Type: application/json" \
  -d '{"prompt": "你好！"}'

# 列出 Agent
curl http://localhost:8080/api/v1/agents \
  -H "Authorization: Bearer <api_key>"
```

### 8.3 Workflow API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/workflows` | 列出工作流 |
| POST | `/api/v1/workflows` | 创建工作流 |
| GET | `/api/v1/workflows/:id` | 获取工作流 |
| DELETE | `/api/v1/workflows/:id` | 删除工作流 |

### 8.4 节点与集群 API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/nodes` | 列出节点 |
| GET | `/api/v1/nodes/:id` | 获取节点详情 |
| GET | `/api/v1/nodes/:id/agents` | 列出节点上的 Agent |
| GET | `/api/v1/cluster/status` | 集群状态概览 |

### 8.5 监控与指标 API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/metrics/summary` | 系统指标汇总 |
| GET | `/api/v1/metrics/agents/:id` | 单个 Agent 指标 |
| GET | `/api/v1/tokens` | Token 使用分析 |
| GET | `/api/v1/scheduler/status` | 调度器状态 |

### 8.6 配置 API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/config` | 获取配置（脱敏） |
| PATCH | `/api/v1/config` | 更新配置（需 Admin） |
| GET | `/api/v1/config/audit-log` | 配置审计日志 |

### 8.7 知识库 API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/knowledge/search` | 搜索知识库 |

### 8.8 A2A（Agent-to-Agent）协议

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/.well-known/agent.json` | Agent Card 发现 |
| GET | `/a2a/v1/agents` | 列出 Agent Cards |
| GET | `/a2a/v1/agents/:id` | 获取 Agent Card |
| POST | `/a2a/v1/tasks` | 发送任务 |
| GET | `/a2a/v1/tasks` | 列出任务 |
| GET | `/a2a/v1/tasks/:id` | 获取任务 |
| DELETE | `/a2a/v1/tasks/:id` | 删除任务 |
| POST | `/a2a/v1/tasks/:id/cancel` | 取消任务 |
| GET | `/a2a/v1/tasks/:id/stream` | 流式任务结果 |

### 8.9 WebSocket

| 路径 | 说明 |
|------|------|
| `/ws` | 实时事件推送 |
| `/api/v1/ws/stats` | WebSocket 连接统计 |

### 8.10 认证方式

- **API Key**：在请求头中添加 `Authorization: Bearer <api_key>`
- **JWT Token**：使用 JWT 签发的 token
- **mTLS**：双向 TLS 认证（配置 `tls_client_ca_path`）

### 8.11 速率限制

默认配置：
- 每秒 10 个请求
- 突发容量 20 个请求

---

## 9. 常见问题解答

### Q1: 服务启动失败，报端口被占用

**问题**：`Error: Address already in use (os error 98)`

**解决**：
```bash
# 查找占用端口的进程
lsof -i :8080

# 修改配置文件中的端口
[api_server]
port = 9090

# 或通过环境变量
export KIAS_API_SERVER__PORT=9090
```

### Q2: Agent 调用返回 401 未授权

**问题**：认证失败

**解决**：
```bash
# 检查 API Key 是否正确
kias config get api_key

# 设置 API Key
kias config set api_key sk-your-correct-key

# 或通过环境变量
export KIAS_API_KEY=sk-your-correct-key
```

### Q3: 本地 Ollama 模型无法连接

**问题**：Agent 调用报连接错误

**解决**：
```bash
# 确认 Ollama 正在运行
ollama list

# 测试连通性
curl http://localhost:11434/api/tags

# 确认配置正确
[model]
provider = "ollama"
endpoint = "http://localhost:11434"
model = "llama3.1:8b"
```

### Q4: 沙箱创建失败

**问题**：Docker 未安装或未启动

**解决**：
```bash
# 检查 Docker 状态
docker info

# 确认用户有 Docker 权限
sudo usermod -aG docker $USER

# 使用 Process 隔离作为替代
kias sandbox create --template python3.11
```

### Q5: 工作流执行卡在某个节点

**问题**：节点长时间 Running 不结束

**解决**：
```bash
# 查看工作流状态
kias workflow status <run_id>

# 查看详细日志
kias workflow logs <run_id>

# 检查 Agent 是否正常
kias agent status --name <agent_name>
```

### Q6: 如何切换不同的 LLM 提供商？

**问题**：想从 OpenAI 切换到 Anthropic

**解决**：

编辑配置文件，只修改 `[model]` 部分：
```toml
[model]
provider = "anthropic"
api_key = "sk-ant-your-key"
model = "claude-3-5-sonnet-20241022"
```

重启服务即可，无需修改任何 Agent 代码。

### Q7: 如何在 CI/CD 中使用 KIAS？

**问题**：需要在自动化流程中调用 Agent

**解决**：
```bash
# 使用 invoke 命令，--text-only 只输出纯文本
kias agent invoke \
  --name code-reviewer \
  --text "审查这段代码的质量" \
  --text-only \
  --timeout 120 \
  --output quiet

# 退出码：0=成功，1=通用错误，2=参数错误，3=服务错误，4=未找到，5=认证错误，6=超时
```

### Q8: 如何监控 Agent 的 Token 消耗？

**解决**：
```bash
# CLI 方式
kias cluster resources

# API 方式
curl http://localhost:8080/api/v1/tokens \
  -H "Authorization: Bearer <api_key>"

# Prometheus 指标
curl http://localhost:9090/metrics
```

### Q9: 如何备份 KIAS 数据？

KIAS 使用 SQLite 作为主存储，备份方法：

```bash
# 备份数据库
cp kias.db kias.db.backup

# 或使用 SQLite 的在线备份
sqlite3 kias.db ".backup 'kias-backup.db'"
```

### Q10: 如何从源码构建？

```bash
# 构建所有 crate
make build

# 运行测试（1400+ 测试）
make test

# 代码检查
make lint

# 性能基准测试
make bench

# 仅构建 CLI
cargo build --release -p kias-cli

# 仅构建服务端
cargo build --release -p kias-main
```

---

## 附录：项目结构

```
kias/
├── crates/
│   ├── kias-cli/          # CLI 命令行工具
│   ├── kias-main/         # 服务主入口
│   ├── api-server/        # HTTP API 服务器
│   ├── controller/        # Agent 生命周期管理
│   ├── scheduler/         # 调度引擎（6 种算法）
│   ├── workflow-engine/   # DAG 工作流引擎
│   ├── model-router/      # 智能模型路由
│   ├── mcp-protocol/      # MCP 协议实现
│   ├── data-store/        # SQLite 数据持久化
│   ├── team-engine/       # Owner-Worker-Verifier 团队引擎
│   ├── langgraph-engine/  # 状态图引擎
│   ├── goal-engine/       # 目标引擎
│   ├── agent-view/        # Agent 可视化
│   ├── autonomy-controller/ # 自治控制器
│   ├── common/            # 公共类型与工具
│   ├── executor/          # 任务执行器
│   ├── knowledge/         # 知识库服务
│   ├── cache/             # 缓存层
│   ├── monitor/           # 监控与指标
│   ├── skills/            # 技能管理
│   └── benchmarks/        # 性能基准测试
├── dashboard/             # Web Dashboard
├── config/                # 配置文件
└── docs/                  # 文档
```

---

<p align="center">
  <sub>Made with ❤️ by the KIAS Team</sub>
</p>
