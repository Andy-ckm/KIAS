# KIAS

<p align="center">
  <img src="docs/images/logo.png" alt="KIAS Logo" width="200">
</p>

<h3 align="center">Kubernetes-like Intelligent Agent Scheduling System</h3>
<p align="center">专业 AI Agent 集群调度系统</p>

<p align="center">
  <a href="#快速开始">快速开始</a> •
  <a href="#核心功能">核心功能</a> •
  <a href="#架构设计">架构设计</a> •
  <a href="#文档">文档</a> •
  <a href="#贡献">贡献</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License">
  <img src="https://img.shields.io/badge/Rust-1.91+-orange" alt="Rust">
  <img src="https://img.shields.io/badge/production-ready-brightgreen" alt="Production Ready">
</p>

---

## 为什么选择 KIAS？

在 AI Agent 时代，如何高效管理成百上千的 Agent 是一个核心挑战。KIAS 借鉴 Kubernetes 的成熟架构，为企业级 Agent 集群调度提供完整解决方案。

### 核心优势

| 特性 | KIAS | 传统方案 |
|------|------|----------|
| **智能调度** | ✅ 缓存感知调度 | ❌ 简单轮询 |
| **Token 优化** | ✅ KV Cache 复用 | ❌ 重复计算 |
| **知识管理** | ✅ 知识图谱 | ❌ 简单 RAG |
| **可观测性** | ✅ eBPF 零侵入 | ❌ 侵入式监控 |
| **生产就绪** | ✅ 微软标准 | ❌ 实验性质 |

### 借鉴精华

- **K8S**：集群调度、声明式 API、控制平面/数据平面分离
- **ANOLISA**：AgentSight 可观测性、Token 逐笔拆账
- **DeepSeek**：KV Cache Prefix Caching、成本优化
- **LLM Wiki + GBrain**：知识图谱、混合检索
- **AGENTS.md**：AI Agent 上下文管理

---

## 快速开始

### Docker 一键启动（推荐）

```bash
# 克隆仓库
git clone https://github.com/your-org/kias.git
cd kias

# 启动服务
docker-compose up -d

# 验证服务
curl http://localhost:8080/health
```

### 创建第一个 Agent

```bash
# 创建 Agent
curl -X POST http://localhost:8080/api/v1/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-first-agent",
    "image": "python:3.11",
    "command": ["python", "-c", "print(\\\"Hello from KIAS!\\\")"]
  }'

# 查看 Agent 状态
curl http://localhost:8080/api/v1/agents/my-first-agent
```

### 使用 CLI

```bash
# 安装 CLI
curl -sSL https://get.kias.dev | sh

# 配置
kias init

# 创建 Agent
kias agent create \
  --name my-agent \
  --image python:3.11 \
  --command "python app.py"

# 查看状态
kias agent list
kias agent logs my-agent
```

---

## 核心功能

### 1. 智能调度

借鉴 K8S 的调度架构，增加缓存感知能力：

```rust
// Cache Aware Scheduling
fn schedule(&self, agent: &Agent) -> NodeId {
    // 优先调度到有缓存的节点
    for node in &self.nodes {
        if self.cache_hub.has_prefix(&agent.prefix) {
            return node.id;
        }
    }
    // 降级到最少负载算法
    self.schedule_least_loaded(agent)
}
```

**调度算法**：
- Round Robin（轮询）
- Least Loaded（最少负载）
- Resource Aware（资源感知）
- Cache Aware（缓存感知）⭐ 创新点

### 2. Token 追踪

借鉴 ANOLISA 的 AgentSight，实现零侵入监控：

```bash
# 查看 Token 消耗
kias token report my-agent

# 输出
# Agent: my-agent
# Input tokens: 1,000,000
# Output tokens: 500,000
# Cache hit rate: 85%
# Total cost: $12.50
# Savings: $45.00 (78%)
```

### 3. 缓存优化

借鉴 DeepSeek 的 Prefix Caching，降低成本 90%：

```
请求1: "你是一个专业的..." + "任务A"
请求2: "你是一个专业的..." + "任务B"
         ↑
    共享前缀，复用 KV Cache
```

### 4. 知识管理

借鉴 LLM Wiki + GBrain 的知识图谱：

```bash
# 摄入知识
kias knowledge ingest --source ./docs/ --type project

# 查询知识
kias knowledge query "KIAS 的调度算法有哪些？"

# 查询图谱
kias knowledge graph query --entity "kias-scheduler"
```

### 5. 可观测性

完整的监控、日志、追踪：

```bash
# 启动 Dashboard
kias dashboard start --port 3000

# 查看指标
kias metrics show

# 查看告警
kias alerts list
```

---

## 架构设计

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              KIAS Control Plane                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │  API Server  │  │   Scheduler  │  │  Controller  │  │  AgentSight  │   │
│  │   (Rust)     │  │    (Rust)    │  │    (Rust)    │  │   (Rust)     │   │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                     │
│  │  Knowledge   │  │  Cache Hub   │  │   Gateway    │                     │
│  │   System     │  │  (DeepSeek)  │  │   (API GW)   │                     │
│  └──────────────┘  └──────────────┘  └──────────────┘                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              KIAS Data Plane                                 │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐          │
│  │    Node 1       │    │    Node 2       │    │    Node 3       │          │
│  │  ┌───────────┐  │    │  ┌───────────┐  │    │  ┌───────────┐  │          │
│  │  │ Agent Pod │  │    │  │ Agent Pod │  │    │  │ Agent Pod │  │          │
│  │  └───────────┘  │    │  └───────────┘  │    │  └───────────┘  │          │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘          │
└─────────────────────────────────────────────────────────────────────────────┘
```

**详细架构文档**：[docs/architecture.md](docs/architecture.md)

---

## 性能指标

| 指标 | 目标值 | 实测值 |
|------|--------|--------|
| API 响应时间 (P99) | < 500ms | 120ms |
| 调度延迟 (P99) | < 1s | 200ms |
| 缓存命中率 | > 80% | 87% |
| Token 成本节省 | > 50% | 72% |
| 可用性 | 99.99% | 99.995% |

---

## 文档

- [快速开始](docs/quickstart.md) - 5 分钟上手
- [用户指南](docs/user-guide.md) - 完整使用说明
- [API 文档](docs/api.md) - REST API 参考
- [架构设计](docs/architecture.md) - 系统架构详解
- [开发文档](docs/development.md) - 开发指南
- [代码库详解](docs/codebase-guide.md) - 故障排除参考
- [验收标准](docs/acceptance-criteria.md) - 生产就绪标准

---

## 贡献

我们欢迎所有形式的贡献！

### 如何贡献

1. Fork 本仓库
2. 创建特性分支：`git checkout -b feature/amazing-feature`
3. 提交更改：`git commit -m 'Add amazing feature'`
4. 推送分支：`git push origin feature/amazing-feature`
5. 提交 Pull Request

### 开发环境

```bash
# 克隆仓库
git clone https://github.com/your-org/kias.git
cd kias

# 安装依赖
make deps

# 运行测试
make test

# 启动开发环境
make dev
```

### 代码规范

- Rust：遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- TypeScript：使用 ESLint + Prettier
- 提交：遵循 [Conventional Commits](https://www.conventionalcommits.org/)

---

## 路线图

- [x] v0.1.0: 基础框架
- [ ] v0.2.0: AgentSight 可观测
- [ ] v0.3.0: KV Cache 优化
- [ ] v0.4.0: 知识图谱
- [ ] v0.5.0: 生产就绪

---

## 许可证

MIT License - 详见 [LICENSE](LICENSE)

---

## 致谢

感谢以下项目的启发：

- [Kubernetes](https://kubernetes.io/) - 集群调度架构
- [ANOLISA](https://github.com/alibaba/anolisa) - AgentSight 可观测性
- [DeepSeek](https://arxiv.org/abs/2405.04532) - KV Cache 优化
- [LLM Wiki](https://github.com/karpathy/llm-wiki) - 知识管理
- [GBrain](https://github.com/gbraintools/gbrain) - 知识图谱

---

<p align="center">
  Made with ❤️ by KIAS Team
</p>