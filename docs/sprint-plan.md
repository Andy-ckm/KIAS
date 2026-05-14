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

## Sprint 2：核心功能（当前）

### 目标
完成核心功能实现，集成参考项目精华

### 开发步骤

| 步骤 | 任务 | 预计时间 | 状态 |
|------|------|----------|------|
| 2.1 | Controller 故障恢复 + 心跳监控 | 3h | ✅ 已完成 |
| 2.2 | WorkflowEngine 节点执行 | 3h | ✅ 已完成 |
| 2.3 | MCP 协议集成 | 3h | ⏳ 待开始 |
| 2.4 | Rig 框架集成 | 4h | ⏳ 待开始 |
| 2.5 | A2A 协议集成 | 4h | ⏳ 待开始 |
|| 2.6 | API Server 集成测试 | 2h | ✅ 已完成 (43 tests) |
|| 2.7 | kias-main 服务编排 | 3h | ✅ 已完成 (27 tests) |
| 2.8 | Knowledge 向量检索 | 3h | ⏳ 待开始 |

### 验收标准（Sprint 2）
- [ ] API 响应 < 200ms (P95)
- [ ] Agent 调度 < 300ms (P95)
- [ ] 测试覆盖率 > 60%（当前 192 测试）
- [ ] MCP 协议基础支持
- [ ] A2A 协议基础支持

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
| 3.7 | 压力测试 | 3h | ⏳ 待开始 |
| 3.8 | DeepSeek MLA Cache 优化 | 6h | ⏳ 待开始 |
| 3.9 | LangGraph 状态图编排 | 4h | ⏳ 待开始 |

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
| A2A 协议 | Google | 跨系统互操作 | 📋 设计中 |
| MCP 协议 | Anthropic | LLM 工具集成 | 📋 设计中 |
| Rig 框架 | Rust 社区 | 原生 AI 能力 | 📋 设计中 |
| DeepSeek MLA | DeepSeek | 显存优化 90% | 📋 设计中 |
| Volcano GPU 调度 | K8S | GPU 利用率 +50% | 📋 设计中 |

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

**Sprint 9 — TLS 1.3 + 安全加固 + 性能优化**

当前状态：931 tests, ~33,800 行(Rust) + 979 行(TS), 0 errors, 0 warnings, 16 crates + 1 前端项目

已完成：
- ✅ TLS 1.3 加密支持（rustls + tokio-rustls + mTLS + 自签名证书）
- ✅ 32 个新 TLS 测试（16 common + 16 api-server）
- ✅ React + TypeScript + Vite + TailwindCSS v4 Dashboard 脚手架
- ✅ API 客户端 + TypeScript 类型系统
- ✅ Dashboard 概览页（集群状态、任务统计）
- ✅ Agents 管理页（列表、创建、删除）
- ✅ Nodes 页面 + Cluster 页面
- ✅ WebSocket 实时推送（EventBus + 9 种事件类型 + 客户端订阅过滤）
- ✅ Agent 协作协议 - CrewAI 风格委托代理（delegation + memory + skill_matcher + crew 模块，55 新测试）

下一步：调度算法优化（K8S descheduler）+ 压力测试 + 性能基准
