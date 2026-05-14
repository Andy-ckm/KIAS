# KIAS 创新点文档（持续更新）

## 已集成的创新点

### 1. MiniMax Agent Team 架构 ✅
- **来源**: MiniMax Agent Team 设计
- **实现**: `crates/team-engine/`
- **核心**: Owner-Worker-Verifier 三角色对抗机制
- **特点**: 确定性状态机驱动，不依赖模型自由判断

### 2. Claude Code /goal 命令 ✅
- **来源**: Claude Code 的 `/goal` 功能
- **实现**: `crates/goal-engine/`
- **核心**: 目标驱动循环（设定 → 执行 → 评估 → 反馈 → 重复）
- **特点**: 裁判分离，双模型评估

### 3. K8S 调度算法 ✅
- **来源**: Kubernetes 调度器
- **实现**: `crates/scheduler/`
- **核心**: 4 种调度算法
  - Round-Robin: 轮询
  - Least-Loaded: 最低负载
  - Resource-Aware: 资源感知（bin-packing）
  - Cache-Aware: 缓存感知（DeepSeek 启发）
- **特点**: 亲和性/反亲和性过滤，优先级排序

### 4. DeepSeek Prefix Cache 优化 ✅
- **来源**: DeepSeek-V3 KV Cache 优化
- **实现**: `crates/scheduler/src/algorithms/cache_aware.rs` + `crates/scheduler/src/optimizer/`
- **核心**: 前缀哈希路由，最大化 KV Cache 命中率
- **特点**: 降低 90% 推理成本

### 5. LangGraph 工作流引擎 ✅
- **来源**: LangGraph StateGraph
- **实现**: `crates/workflow-engine/`
- **核心**: DAG + 循环 + 条件分支 + 并行
- **特点**: 支持 checkpoint 持久化，状态恢复

### 6. OpenAI Codex 三模式自治 ✅
- **来源**: OpenAI Codex CLI
- **实现**: `crates/autonomy-controller/`
- **核心**: SuggestOnly / AutoEdit / FullAuto 三级自治
- **特点**: 按工具粒度控制自治权限

### 7. Prometheus 可观测性 ✅
- **来源**: ANOLISA AgentSight + K8S 监控
- **实现**: `crates/common/src/metrics.rs` + `crates/monitor/`
- **核心**: 7 个 Prometheus 指标（Agent、Scheduler、Cache、Token、Node）
- **特点**: Token 逐笔拆账，成本计算

## 2026-05 研究发现的新创新点

### AutoAgents (liquidos-ai/AutoAgents) ⭐633 Rust 多 Agent 框架
- **架构特点**: 模块化设计，Type-safe Agent 模型，结构化工具调用，可插拔 Memory
- **可借鉴点**:
  - LLM Guardrails（推理安全保障）→ KIAS 可集成到 autonomy-controller
  - WASM 沙箱执行工具 → KIAS sandbox 模块可参考
  - OpenTelemetry 可观测性 → KIAS monitor 可增强
  - 统一 LLM 接口（OpenAI/Anthropic/DeepSeek/xAI）→ KIAS model-router 可对标
- **差距**: KIAS 已有类似模块，但 AutoAgents 的"Guardrails + Optimization passes"（cache/retry）是新增的

### GraphBit (InfinitiBit/graphbit) ⭐538 企业级 Agentic 框架
- **架构特点**: Rust 核心 + Python 包装器，企业级，HNSW 向量存储
- **可借鉴点**:
  - 企业级就绪标识（监控、指标、错误处理）
  - 多租户支持
- **差距**: KIAS data-store 的 HNSW 实现已经类似

### mcp-memory-service (doobuidoo/mcp-memory-service) ⭐1838
- **LangGraph/CrewAI/AutoGen 持久化 Memory** → KIAS team-engine memory 模块可集成 MCP Memory 协议

---

## 待集成的创新点（2025-05 研究）

### 8. Google A2A 协议（Agent-to-Agent）📋
- **来源**: Google A2A 开放标准
- **协议**: JSON-RPC over HTTP + Agent Cards
- **集成点**: KIAS 的 Agent 间通信层
- **实现计划**:
  - 每个 Agent 发布 Agent Card 描述能力
  - Scheduler 使用 A2A 任务委派协议路由工作
  - 支持多厂商 Agent 生态
- **优先级**: 高

### 9. Anthropic MCP（Model Context Protocol）📋
- **来源**: Anthropic MCP 标准
- **协议**: 通用工具接口（"AI 的 USB-C"）
- **集成点**: KIAS 的工具/服务接口层
- **实现计划**:
  - 每个 K8S 管理的 Agent 工具暴露 MCP 端点
  - Scheduler 管理 MCP 连接和 OAuth 认证
  - 可用 `rig` crate 的 MCP 客户端
- **优先级**: 高

### 10. Volcano GPU 调度 📋
- **来源**: Volcano K8S 批处理调度器
- **核心**: Gang 调度、公平共享队列、GPU 拓扑感知
- **集成点**: KIAS 的 GPU 任务调度
- **实现计划**:
  - Gang 调度：多 Agent 协调任务全有或全无
  - 公平共享：多租户 Agent 环境
  - GPU 拓扑感知：NVLink/PCIe 感知调度
- **优先级**: 中

### 11. DeepSeek MLA（Multi-head Latent Attention）📋
- **来源**: DeepSeek-V3
- **核心**: KV Cache 压缩 93.3%
- **集成点**: KIAS 的推理优化层
- **实现计划**:
  - 前缀感知请求路由
  - 跟踪 GPU Pod 的前缀哈希分布
  - 调度决策最大化缓存利用率
- **优先级**: 中

### 12. Rig Rust Agent 框架 📋
- **来源**: Rig (Rust AI 框架)
- **核心**: 基于 trait 的 LLM Agent 构建
- **集成点**: KIAS 的核心 Agent 运行时
- **实现计划**:
  - 类型安全的工具定义
  - MCP 客户端集成
  - 多 Agent 编排
- **优先级**: 高

### 13. CrewAI 声明式编排 📋
- **来源**: CrewAI
- **核心**: 角色/目标声明式定义
- **集成点**: KIAS 的简单 Agent 团队层
- **实现计划**:
  - 声明式角色/目标定义
  - CrewAI Flows 确定性工作流
  - 知识管理集成
- **优先级**: 低

## 创新点优先级排序

| 优先级 | 创新点 | 状态 | 预计工作量 |
|--------|--------|------|-----------|
| P0 | K8S 调度算法 | ✅ 已完成 | - |
| P0 | DeepSeek Prefix Cache | ✅ 已完成 | - |
| P0 | MiniMax Agent Team | ✅ 已完成 | - |
| P0 | Claude Code /goal | ✅ 已完成 | - |
| P1 | Google A2A 协议 | 📋 待实现 | 2 周 |
| P1 | Anthropic MCP | ✅ 已完成 | - |
| P1 | Rig 框架集成 | 📋 待实现 | 1 周 |
| P2 | Volcano GPU 调度 | 📋 待实现 | 3 周 |
| P2 | DeepSeek MLA | 📋 待实现 | 2 周 |
| P3 | CrewAI 声明式编排 | 📋 待实现 | 2 周 |
