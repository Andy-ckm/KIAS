# KIAS 循环开发计划

## 循环开发流程

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│   │  开发    │───▶│  测试    │───▶│  修复    │───▶│  创新    │  │
│   └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
│        ▲                                                 │      │
│        └─────────────────────────────────────────────────┘      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Sprint 1：基础框架 ✅ 已完成

### 验收结果
- [x] `cargo check` 无错误
- [x] `cargo test` 113 → 192 测试通过
- [x] 15 个 crate 全部编译通过
- [x] 核心类型系统建立

---

## Sprint 14：Data Layer Architecture + LangGraph Engine ✅ 已完成

### 目标
数据层架构：SQLite Repository + HNSW vector storage + Cache + Experience Replay + PrefixCache

### 开发步骤

| 步骤 | 任务 | 预计时间 | 状态 |
|------|------|----------|------|
| 2.1 | Controller 故障恢复 + 心跳监控 | 3h | ✅ 已完成 |
| 2.2 | WorkflowEngine 节点执行 | 3h | ✅ 已完成 |
| 2.3 | MCP 协议集成 | 3h | ✅ 已完成 (mcp-protocol crate, sandbox, tool hot-reload, 30+ tests) |
| 2.4 | Rig 框架集成 | 4h | ⏸️ 已跳过 (自研 model-router 更轻量) |
| 2.5 | A2A 协议集成 | 4h | ✅ 已完成 (HTTP API + SSE streaming + 13 tests) |
| 2.6 | API Server 集成测试 | 2h | ✅ 已完成 (43 tests) |
| 2.7 | kias-main 服务编排 | 3h | ✅ 已完成 (27 tests) |
| 2.8 | Knowledge 向量检索 | 3h | ✅ 已完成 (HNSW + Exact 混合搜索, 24 tests) |
| 2.9 | data-store SQLite Repository | 3h | ✅ 已完成 (Repository trait + 8 models + 迁移系统) |
| 2.10 | data-store HNSW vector persist | 3h | ✅ 已完成 (SQLite write-through + HNSW in-memory, 4 tests) |
| 2.11 | data-store Cache Strategy | 2h | ✅ 已完成 (TTL + 命名空间隔离, 9 tests) |
| 2.12 | data-store Experience Replay | 2h | ✅ 已完成 (batch insert + episode 追踪 + 随机采样) |
| 2.13 | data-store PrefixCache | 2h | ✅ 已完成 (DeepSeek 风格 KV 缓存 + hit tracking + LRU) |

### 验收标准（Sprint 14）
- [x] SQLite Repository trait + SqliteRepository 实现
- [x] 8 个数据模型（Agent, Task, Workflow, Config, Skill, Component, ExperienceReplay, PrefixCache）
- [x] HNSW vector search (kias-knowledge VectorStore 实现, O(log N) ANN)
- [x] HNSW ANN search: all index sizes use O(log N) HNSW (no O(N) fallback)
- [x] Cache strategy: TTL + 命名空间隔离
- [x] Experience Replay: batch insert + episode 追踪
- [x] Prefix Cache: DeepSeek 风格 token-level KV cache
- [x] 迁移系统：4 个迁移 (core, vector, cache, experience_replay)
- [x] 测试覆盖：1215 tests (从 1047 → 1198, +14%)

### 验收标准（Sprint 2）
- [ ] API 响应 < 200ms (P95)
- [ ] Agent 调度 < 300ms (P95)
- [ ] 测试覆盖率 > 60%（当前 1198 测试）
- [x] MCP 协议基础支持 ✅ (Sprint 14 mcp-protocol crate, 30+ tests)
- [x] A2A 协议基础支持 ✅ (Sprint 14 A2A HTTP API + SSE streaming, 13 tests)

### 当前进度
- ✅ Controller 从 30% → 85%
- ✅ WorkflowEngine 从 55% → 90%
- ✅ MCP 协议框架完成 (30 tests)
- ✅ API Server 集成测试 (43 tests)
- ✅ kias-main 服务编排 (27 tests)
- ✅ 测试从 113 → 292 (+158%)
- ✅ 代码量从 6,945 → 12,863 行 (+85%)
- ✅ 编译警告清零
- ✅ Descheduler 集群重平衡 (25 tests, 3 strategies + PDB + dry-run)

---

## Sprint 3：生产就绪（第三周）

### 目标
达到微软标准，通过验收测试

### 开发步骤

| 步骤 | 任务 | 预计时间 | 状态 |
|------|------|----------|------|
| 3.1 | OAuth2/JWT 认证 | 3h | ✅ 已完成 (Sprint 6) |
| 3.2 | RBAC 权限控制 | 3h | ✅ 已完成 (Sprint 6) |
| 3.3 | TLS 1.3 加密 | 2h | ✅ 已完成 (Sprint 9) |
| 3.4 | 数据脱敏 | 2h | ✅ 已完成 (Sprint 6) |
| 3.5 | 审计日志 | 2h | ✅ 已完成 (Sprint 6) |
| 3.6 | Prometheus + Grafana 集成 | 4h | 🔶 部分完成 (Sprint 5: 指标端点 + 告警引擎已实现) |
| 3.7 | 压力测试 + 性能基准 | 3h | ✅ 已完成 (Sprint 10: Criterion benchmarks + concurrent stress) |
| 3.8 | DeepSeek MLA Cache 优化 | 6h | ✅ 已完成 (refs: references/deepseek-mla-cache-pattern.md) |
| 3.9 | LangGraph 状态图编排 | 4h | ✅ 已完成 (crates/langgraph-engine, 状态图引擎 + 创新功能) |

### 验收标准（Sprint 3）
- [ ] API QPS > 10,000
- [ ] Agent 并发 > 1,000
- [ ] CPU 峰值 < 90%
- [ ] 内存峰值 < 80%
- [ ] 安全扫描通过

---

## Sprint 4：创新功能（第四周）

### 目标
超越竞品，建立技术壁垒

### 创新点

| 创新点 | 参考来源 | 预计收益 | 状态 |
|--------|----------|----------|------|
| Cache Aware Scheduling | DeepSeek | 成本降低 90% | ✅ 已集成 |
| 故障自动恢复 | K8S | 可用性 99.99% | ✅ 已集成 |
| DAG 工作流引擎 | Temporal | 复杂任务编排 | ✅ 已集成 |
| A2A 协议 | Google | 跨系统互操作 | ✅ 已完成 (HTTP API + SSE + routing) |
| MCP 协议 | Anthropic | LLM 工具集成 | ✅ 已完成 (mcp-protocol crate, sandbox, tool hot-reload, 30+ tests) |
| Rig 框架 | Rust 社区 | 原生 AI 能力 | 📋 设计中 |
| DeepSeek MLA | DeepSeek | 显存优化 90% | ✅ 已完成 (refs: references/deepseek-mla-cache-pattern.md) |
| LangGraph 状态图 | LangChain | 复杂任务编排 | ✅ 已完成 (crates/langgraph-engine) |

---

## 汇报计划

| 时间 | 汇报内容 |
|------|----------|
| 每 15 分钟 | 当前任务进度、遇到的问题 |
| 每小时 | 完成任务数、测试结果 |
| 每天 | Sprint 进度、风险评估 |
| 每周 | 里程碑完成情况、创新点进展 |

---

## 参考源码

| 项目 | Stars | 借鉴内容 | 状态 |
|------|-------|----------|------|
| kubernetes/kubernetes | 110k+ | 集群调度、声明式 API | ✅ 已下载 |
| alibaba/anolisa | 100+ | eBPF 监控、AgentSight | ✅ 已下载 |
| temporalio/temporal | 12k+ | 工作流引擎、DAG 执行 | 📋 参考中 |
| langchain-ai/langgraph | 8k+ | 状态图编排 | 📋 参考中 |
| DeepSeek | - | KV Cache 优化 | 📋 设计文档 |
| rig-rs/rig | 2k+ | Rust AI 框架 | 📋 参考中 |

---

## 当前任务

**Sprint 14 — Data Layer Architecture + LangGraph State Graph Engine + Innovations**

当前状态：1464 tests, 75,324 行(Rust) + 2,392 行(Dashboard), 0 errors, 0 warnings, 21 crates + 1 前端项目

已完成：
- ✅ kias-data-store crate 完整实现（L1 架构层）
- ✅ SQLite Repository 抽象层（Repository<T> trait + SqliteRepository facade）
- ✅ 8 个数据模型（Agent, Task, Workflow, Config, Skill, Component, ExperienceReplay, PrefixCache）
- ✅ 4 个迁移（core tables, vector, cache, experience replay + prefix cache）
- ✅ 向量持久化存储（SQLite + DashMap write-through）
- ✅ 缓存策略（TTL + 命名空间隔离 + 访问计数）
- ✅ Experience Replay 存储（batch insert, episode 追踪, 随机采样）
- ✅ Prefix Cache 存储（DeepSeek 风格 KV 缓存, hit tracking, LRU eviction）
- ✅ 健康检查 + 连接池统计
- ✅ 集成到 KiasServiceManager（kias-main）
- ✅ 设计文档：docs/design-docs/data-layer-architecture.md
- ✅ LangGraph 状态图引擎 (crates/langgraph-engine)
- ✅ DeepSeek MLA Cache 优化 (references/deepseek-mla-cache-pattern.md)
- ✅ 1215 tests passing, 0 clippy warnings, lint-arch OK

下一步：前端 Agent 详情页 / Volcano GPU 调度 / 其他创新功能

Sprint 16 更新 (2026-05-15):
- ✅ model-router 测试扩展: 18 → 55 tests (+37, +206%)
- ✅ 修复 RequestCache::get DashMap 死锁 (read guard + write guard 冲突)
- ✅ 代码质量: 0 clippy warnings, 0 test failures
- ✅ 创新调研: Rust Agent 生态 (openfang 17K⭐, rig 7K⭐, DeepSeek-TUI 29K⭐)
- ✅ sprint-progress.md 更新到 Sprint 16

Sprint 28 更新 (2026-05-16):
- ✅ 1464 tests passing, 0 clippy warnings, fmt clean, 0 pedantic warnings
- ✅ 75,324 lines Rust code across 21 crates + 2,392 lines Dashboard
- ✅ 71+ innovation points tracked
- ✅ 所有优先级验证完成 (HNSW, Redis清理, MCP, Data Layer)
- ✅ fmt 修复: kias-main/src/main.rs 排序问题
