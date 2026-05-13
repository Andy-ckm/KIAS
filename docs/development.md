# KIAS 开发文档（持续更新）

## 项目概述

KIAS (Kubernetes-Inspired Agent System) 是一个企业级 AI Agent 集群系统。

## 核心设计（借鉴来源）

### 1. MiniMax Agent Team
- Owner-Worker-Verifier 架构
- 对抗性质量门禁
- 确定性状态机驱动
- 上下文隔离

### 2. Claude Code /goal
- 目标驱动循环
- 裁判分离（Worker 和 Verifier 独立）
- 双模型评估
- 好目标三要素：可衡量的终态、验证方式、约束

### 3. Ralph Loop
- 设定目标就开跑
- 失败就重来，直到达成
- 跨会话保持目标

### 4. 编程即训练
- model.fit() = /goal
- 需求文档 = Loss Function
- 测试用例 = 验证集
- Agent 迭代 = Training Step

## 项目结构

```
kias/
├── crates/
│   ├── common/          # 基础库 (L0) — KiasError, types, config, metrics, utils, messaging, **a2a**
│   ├── api-server/      # REST API (L2) — axum handlers, middleware, routes
│   ├── scheduler/       # 调度引擎 (L2) — 4 算法 + 亲和性 + 优先级 + 缓存优化 + **Edge Scheduling**
│   ├── controller/      # 控制器 (L2) — 状态调和 + **Agent Handoff**
│   ├── knowledge/       # 知识图谱 (L2) — 图存储 + 检索 + Agent Memory System
│   ├── cache/           # 缓存系统 (L2) — LRU + 前缀缓存策略
│   ├── monitor/         # 可观测性 (L2) — 遥测 + 指标
│   ├── executor/        # 任务执行器 (L2) — TaskRuntime + Sandbox Executor
│   ├── skills/          # 技能注册中心 (L2) — Skill trait + registry
│   ├── team-engine/     # Team 引擎 (L2) — Owner-Worker-Verifier + Swarm Orchestrator
│   ├── goal-engine/     # 目标引擎 (L2) — /goal 循环 + RoundExecutor
│   ├── workflow-engine/ # 工作流引擎 (L2) — LangGraph-style DAG + **Durable Replay**
│   ├── autonomy-controller/ # 自治控制器 (L2) — Codex 三模式
│   ├── agent-view/      # Agent 视图 (L2) — 会话展示
│   └── kias-main/       # 主入口 (L3) — 服务编排
├── docs/
└── references/
```

## 开发进度

### Sprint 1：基础框架 ✅

| 任务 | 状态 | 完成时间 |
|------|------|----------|
| 创建项目结构 (16 crates) | ✅ | 2025-05-13 23:00 |
| 集成 MiniMax Agent Team 设计 | ✅ | 2025-05-14 00:20 |
| 集成 Claude Code /goal 设计 | ✅ | 2025-05-14 00:25 |
| 集成 Ralph Loop 设计 | ✅ | 2025-05-14 00:30 |
| 修复编译错误（全部 crates） | ✅ | 2026-05-14 01:20 |
| cargo check 无错误 | ✅ | 2026-05-14 01:20 |

### Sprint 2：测试 + 质量 ✅

| 任务 | 状态 | 完成时间 |
|------|------|----------|
| 添加单元测试（113 个测试） | ✅ | 2026-05-14 02:50 |
| cargo clippy 0 warnings | ✅ | 2026-05-14 02:50 |
| goal-engine 回调式执行模型 | ✅ | 2026-05-14 02:50 |
| agent-view 会话管理测试 | ✅ | 2026-05-14 02:50 |

### Sprint 3：创新功能 ✅

| 任务 | 状态 | 完成时间 |
|------|------|----------|
| 搜索创新点 (Plano/microsandbox/golutra/cersei) | ✅ | 2026-05-14 04:30 |
| Agent Message Bus (common/messaging) | ✅ | 2026-05-14 04:35 |
| Sandbox Executor (executor/sandbox) | ✅ | 2026-05-14 04:40 |
| Agent Memory System (knowledge/memory) | ✅ | 2026-05-14 04:45 |
| Swarm Orchestrator (team-engine/swarm) | ✅ | 2026-05-14 04:50 |
| 修复所有 clippy warnings | ✅ | 2026-05-14 04:55 |
| 测试数量：292 → 332 | ✅ | 2026-05-14 04:55 |
| 代码行数：12,248 → 14,006 | ✅ | 2026-05-14 04:55 |

### Sprint 4：A2A + 持久执行 + 边缘调度 ✅

| 任务 | 状态 | 完成时间 |
|------|------|----------|
| A2A Protocol（common/a2a） | ✅ | 2026-05-14 05:10 |
| Durable Execution Replay（workflow-engine/replay） | ✅ | 2026-05-14 05:15 |
| Agent Handoff（controller/handoff） | ✅ | 2026-05-14 05:20 |
| Edge Node Scheduling（scheduler/edge） | ✅ | 2026-05-14 05:25 |
| 测试数量：332 → 366 | ✅ | 2026-05-14 05:25 |
| 代码行数：14,006 → 16,506 | ✅ | 2026-05-14 05:25 |
| cargo clippy 0 warnings | ✅ | 2026-05-14 05:25 |

### 测试覆盖（366 个测试）

| Crate | 测试数 | 覆盖内容 |
|-------|--------|----------|
| common | **26** | error, config, utils, metrics, logging, **A2A Protocol** |
| scheduler | **39** | 算法(RR/LL/RA/CA), 亲和性, 优先级, 优化器, **Edge Scheduling** |
| controller | **59** | 心跳监控, 故障恢复, 健康检查, 状态管理, 调和器, **Agent Handoff** |
| workflow-engine | **43** | DAG 引擎, 节点执行(Shell/HTTP/LLM), 条件分支, 重试, 检查点, **Replay** |
| team-engine | 25 | Owner-Worker-Verifier + Swarm Orchestrator |
| autonomy-controller | 8 | 三模式, 工具策略, 梯度配置 |
| monitor | 8 | 遥测事件, 指标收集 |
| goal-engine | 7 | 目标创建, 评估器, 条件检查 |
| agent-view | 7 | 视图创建, 会话管理, 状态转换 |
| skills | 5 | 注册, 查询, 列表, 执行 |
| cache | 4 | LRU set/get/evict, CacheHub |
| knowledge | 17 | 图节点, 边, 邻居查询 + Agent Memory System |
| executor | 13 | 任务创建, 状态, 结果 + Sandbox Executor |
| mcp-protocol | 30 | MCP 客户端/服务端, 工具/资源/Prompt |
| api-server | 43 | REST API 集成测试 |
| **总计** | **366** | **366/366 全部通过** |

## 创新点研究（2026-05-14 更新）

### 发现的参考项目

| 项目 | Stars | 创新点 | 对KIAS的启发 |
|------|-------|--------|-------------|
| **Plano** | 6.4k | AI-native proxy + data plane | Agent 网络层设计 |
| **microsandbox** | 6k | 安全沙箱执行 | Sandbox Executor ✅ |
| **golutra** | 3.4k | 多Agent编排平台 | Swarm Orchestrator ✅ |
| **Ralph Orchestrator** | 2.8k | 改进 Ralph Loop | 已集成 |
| **Chidori** | 1.3k | 持久化 Agent 运行时 | Durable Replay ✅ NEW |
| **hcom** | 272 | Agent 间通信 | Agent Message Bus ✅ |
| **cersei** | 287 | Rust Agent SDK | 工具执行 + 图记忆 |
| **python-a2a** | 989 | Google A2A 协议实现 | A2A Protocol ✅ NEW |
| **Agent-MCP** | 1.2k | 多Agent协作 MCP | 知识图谱 + 任务管理 |
| **LightAgent** | 968 | 轻量级 Agent 框架 | 记忆 + MCP + 技能 |

### 已集成的创新设计（16 项）

1. **对抗性质量门禁**（MiniMax）— Worker-Verifier 对抗
2. **确定性状态机驱动**（MiniMax）— 不依赖模型自由判断
3. **裁判分离**（Claude Code）— Worker 和 Verifier 独立
4. **双模型评估**（Claude Code）— 评估模型独立于执行模型
5. **目标驱动循环**（Ralph Loop）— 设定目标就开跑
6. **编程即训练**（François Chollet）— model.fit() = /goal
7. **回调式执行器**（新增）— RoundExecutor trait 支持可插拔执行
8. **自主度梯度**（Codex）— Suggest/AutoEdit/FullAuto 三模式
9. **Agent Message Bus**（hcom）— Pub/Sub + 直接通信
10. **Sandbox Executor**（microsandbox）— 沙箱隔离执行
11. **Agent Memory System**（Memory Palace）— 三层记忆模型
12. **Swarm Orchestrator**（golutra）— 4种编排策略
13. **A2A Protocol**（Google）— Agent-to-Agent 标准化通信 ✅ NEW
14. **Durable Execution Replay**（Chidori）— 确定性检查点回放 ✅ NEW
15. **Agent Handoff**（A2A/Plano）— 任务自动转移 + 候选选择 ✅ NEW
16. **Edge Node Scheduling**（YoMo/K8S）— 边缘/雾/物联网节点调度 ✅ NEW

### 下一步创新方向

1. **Observability Dashboard** — 参考 ANOLISA，实现 Agent 可视化
2. **Agent-MCP 协作** — 参考 Agent-MCP，增强多 Agent 知识共享
3. **Streaming Responses** — 参考 A2A 流式响应，支持实时输出
4. **Push Notifications** — 参考 A2A 推送通知，异步任务回调

## 验收标准

- [x] `cargo check` 无错误 ✅
- [x] `cargo test` 全部通过 (366/366) ✅
- [x] `cargo clippy` 0 warnings ✅
- [ ] 代码覆盖率 > 60%
- [x] API 集成测试 (43 tests) ✅
- [ ] 端到端工作流测试

## 修复记录

| 修复项 | 影响 crate | 说明 |
|--------|-----------|------|
| KiasResult import 路径 | 11 个文件 | `kias_common::error::KiasResult` → `kias_common::KiasResult` |
| Scheduler 重复模块 | scheduler | `strategy.rs` + `strategy/` 冲突 → 合并为 `strategy/mod.rs` |
| resource_aware 类型错误 | scheduler | `0.0` → `0` (u64 比较) |
| affinity 生命周期 | scheduler | `&'a [&'a Node]` → `[&'a Node]` |
| Executor 返回类型 | executor | SimpleExecutor 返回 TaskResult 而非 Value |
| Cargo 注册表配置 | 全局 | 移除不可用的 rsproxy 镜像，使用 Tsinghua 镜像 |
| clippy derivable_impls | common, cache, etc. | 手动 Default impl → #[derive(Default)] |
| clippy new_without_default | 12 crates | 添加 Default impl |
| clippy sort_by_key | scheduler | sort_by → sort_by_key + Reverse |
| TaskStatus PartialEq | executor | 添加 PartialEq, Eq derive |
| mcp-protocol type_complexity | mcp-protocol | 添加 ToolHandler 类型别名 |
| swarm div_ceil | team-engine | 手动除法 → .div_ceil() |
| edge schedule 生命周期 | scheduler | 添加 `'a` 生命周期参数 |
| select_candidate 生命周期 | controller | 添加 `'a` + `move` 闭包 |
| temporary value dropped | controller | 测试中绑定数组到 let 变量 |
| unused import HashMap | workflow-engine | 移除未使用的 HashMap import |
| derivable Default | scheduler | EdgeSchedulingConstraints 使用 #[derive(Default)] |
