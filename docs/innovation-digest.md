# AgentGuard 创新摘要 — AI Agent 框架 2025-2026 前沿调研

> **生成日期**: 2026-05-16
> **调研范围**: LangGraph v1.2, CrewAI v1.14, AutoGen v0.7, OpenAI Agents SDK v0.17, Google ADK v1.33, Semantic Kernel v1.42
> **筛选标准**: 生产可用 ✅ | AgentGuard 尚未实现 ✅ | Rust 可行 ✅ | 至少 5 个具体创新点

---

## 创新点总览

| # | 创新点 | 来源 | 实现难度 | 优先级 |
|---|--------|------|----------|--------|
| 1 | 节点级错误处理 + 宿主崩溃恢复 | LangGraph v1.2 | ⭐⭐⭐ 中 (12h) | P0 |
| 2 | 会话历史压缩 (Session Compaction) | OpenAI Agents SDK v0.16 | ⭐⭐ 低 (8h) | P0 |
| 3 | Human-in-the-Loop 审批门禁 | CrewAI v1.14 / OpenAI SDK | ⭐⭐ 低 (8h) | P1 |
| 4 | Agent-as-Tool 模式 | AutoGen v0.7 | ⭐⭐⭐ 中 (10h) | P1 |
| 5 | OpenTelemetry GenAI 标准追踪 | AutoGen v0.6 / Google ADK | ⭐⭐⭐ 中 (10h) | P2 |
| 6 | 可调用条件边 (Callable Condition) | AutoGen v0.6 | ⭐⭐ 低 (8h) | P1 |
| 7 | 会话缓冲服务 (Bufferable Session) | Google ADK v1.33 | ⭐⭐⭐ 中 (8h) | P2 |
| 8 | 流式转换器 (Streaming Transformer) | LangGraph v1.2 | ⭐⭐⭐ 中 (10h) | P1 |
| 9 | Flow 声明式持久化 | CrewAI v1.14 | ⭐⭐⭐ 中 (10h) | P2 |
| 10 | A2A 协议标准支持 | Google A2A / CrewAI v1.14 | ⭐⭐⭐⭐ 高 (12h) | P3 |

---

## 详细创新点

### 1. 节点级错误处理 + 宿主崩溃恢复

**来源**: LangGraph v1.2.0 (2026-05-12), PR #7773, #7233
**原理**: 每个 StateGraph 节点可注册独立的 `error_handler`，失败时由 handler 决定重试/跳过/降级路由，而非中断整个图。配合 Durable Resume 机制，通过 checkpoint writes history 实现崩溃后自动恢复。
**AgentGuard 差距**: 当前错误处理是引擎级的，不是节点级；检查点是全量快照无增量持久化。
**实现方案**: 在 `workflow-engine/src/node.rs` 增加 `ErrorHandler` trait，检查点改为 WAL 模式，engine 启动时自动 resume 未完成任务。
**难度**: ⭐⭐⭐ 中等 (12h) | **收益**: 🔴 生产可靠性关键提升

### 2. 会话历史压缩 (Session Compaction)

**来源**: OpenAI Agents SDK v0.16 (2026-05)
**原理**: 当对话历史超过 token 限制时自动压缩旧消息。保留最近消息不变，将早期消息用 LLM 摘要化。压缩失败时自动恢复原始历史。支持 reasoning 内容持久化用于审计。
**AgentGuard 差距**: team-engine 有三层记忆系统但无压缩能力，长对话易超 token 限制。
**实现方案**: 在 `team-engine/src/memory/compaction.rs` 实现 `CompactionStrategy` trait，支持 SlidingWindow 和 SummarizeOld 两种策略，集成 token 计数器自动触发压缩。
**难度**: ⭐⭐ 低 (8h) | **收益**: 🔴 降低 LLM 成本，长对话必备

### 3. Human-in-the-Loop 审批门禁

**来源**: CrewAI v1.14.2 (2026-04), OpenAI Agents SDK v0.16
**原理**: CrewAI Flow 支持 pre-review（预审）和 distillation（精炼确认）两种 HITL 模式。OpenAI SDK 通过 `approval_func` 回调决定是否批准工具调用，支持审批拒绝原因审计。
**AgentGuard 差距**: 有简单 HumanApproval 节点，但无 pre-review、审批策略 DSL、审批历史追踪。
**实现方案**: 在 `workflow-engine/src/approval.rs` 增加 `ApprovalPolicy` 枚举（Always/Threshold/AutoApprove/HumanReview），审批上下文传递，决策持久化到审计日志。
**难度**: ⭐⭐ 低 (8h) | **收益**: 🟡 生产级人机协作

### 4. Agent-as-Tool 模式

**来源**: AutoGen v0.7 (2025-08), Agent-as-Tool / TeamTool
**原理**: Agent 可被封装为标准工具供其他 Agent 调用。调用方通过标准工具接口触发被封装 Agent，执行完成后结果作为返回值传回。支持 TeamTool 模式——将 Agent 团队封装为单个工具。
**AgentGuard 差距**: team-engine 支持 Owner-Worker-Verifier 但无 "Agent 注册为工具" 能力，无法层级嵌套。
**实现方案**: 在 `team-engine/src/agent_tool.rs` 实现 `AgentTool` 包装器，实现 `Tool` trait，注册到 skill registry，支持 TeamTool 组合模式。
**难度**: ⭐⭐⭐ 中等 (10h) | **收益**: 🟡 Agent 能力可组合，层级化架构

### 5. OpenTelemetry GenAI 标准追踪

**来源**: AutoGen v0.6.2 (GenAI Semantic Convention), Google ADK v1.32 (native OTel metrics)
**原理**: 遵循 OpenTelemetry GenAI Semantic Convention 标准 span：`gen_ai.agent.create`、`gen_ai.agent.invoke`、`gen_ai.tool.execute`。标准化 trace 格式可无缝对接 Jaeger/Grafana Tempo。
**AgentGuard 差距**: 有 Prometheus 指标和基础 tracing，但未遵循 GenAI Semantic Convention，无 Agent 级 trace span。
**实现方案**: 在 `common/src/tracing/genai_spans.rs` 实现标准 span，集成 opentelemetry-rust SDK，支持 `AgentGuard_OTEL_ENABLED` 环境变量控制。
**难度**: ⭐⭐⭐ 中等 (10h) | **收益**: 🟢 标准化可观测性，跨框架互操作

### 6. 可调用条件边 (Callable Condition)

**来源**: AutoGen v0.6.0 (2025-06), PR #6623
**原理**: 条件边支持传入 lambda/callable 作为条件，替代简单关键字子串匹配。条件函数接收当前状态返回布尔值，支持 DAG 拓扑并行/串行执行。
**AgentGuard 差距**: langgraph-engine 有条件边但基于字符串匹配/枚举路由，无自定义闭包机制。
**实现方案**: 在 `langgraph-engine/src/condition.rs` 实现 `ConditionEvaluator` trait，内置 RegexMatch/JsonPath/NumericCompare/CustomScript 条件类型。
**难度**: ⭐⭐ 低 (8h) | **收益**: 🟡 工作流路由逻辑极大增强

### 7. 会话缓冲服务 (Bufferable Session)

**来源**: Google ADK v1.33.0 (2026-05-08)
**原理**: 会话状态读写缓冲在内存中，批量刷新到持久化存储。支持 Timer-based/Size-based/Manual 三种 flush 策略，崩溃时通过 WAL 恢复未刷新数据。
**AgentGuard 差距**: data-store 直接读写 SQLite，每次状态变更触发磁盘 I/O，高并发下成为瓶颈。
**实现方案**: 在 `data-store/src/buffer.rs` 实现 `SessionBuffer`，使用 DashMap 做并发安全写缓冲，WAL 保障崩溃安全，与现有 SQLite store 透明集成。
**难度**: ⭐⭐⭐ 中等 (8h) | **收益**: 🟢 高并发 I/O 性能提升 5-10x

### 8. 流式转换器 (Streaming Transformer)

**来源**: LangGraph v1.2.0 (2026-05-12), PR #7519, #7677
**原理**: 在图执行过程中对流式输出进行实时转换和过滤。支持 `stream_events(v3)` 协议，token 级别输出，多种投影模式（custom/updates/checkpoints/debug/tasks）。
**AgentGuard 差距**: workflow-engine 和 langgraph-engine 都是 "执行完返回结果" 模式，无流式输出。
**实现方案**: 定义 `StreamEvent` 枚举和 `StreamTransformer` trait，使用 tokio::sync::broadcast channel，通过 SSE/WebSocket 推送。
**难度**: ⭐⭐⭐ 中等 (10h) | **收益**: 🟡 LLM 任务实时流式体验

### 9. Flow 声明式持久化

**来源**: CrewAI v1.14.4 (2026-04), PR #5649
**原理**: 通过 `@persist` 装饰器标记需要持久化的状态字段，支持自定义 persistence key。Flow 任意步骤可保存快照，从指定快照继续执行。
**AgentGuard 差距**: workflow-engine 有 checkpoint 但无声明式持久化、无按实例快照管理、无 UI 层 checkpoint 选择。
**实现方案**: 实现 `#[derive(Persistable)]` 宏，`StateStore` trait，自动快照 + API 端点查看/恢复。
**难度**: ⭐⭐⭐ 中等 (10h) | **收益**: 🟢 长工作流可回溯调试

### 10. A2A 协议标准支持

**来源**: Google A2A Protocol (2025), CrewAI v1.14.2
**原理**: 每个 Agent 暴露标准化 Agent Card（JSON 描述），声明能力/端点/认证方式。Agent 间通过 A2A 协议委托任务。与 MCP 互补：MCP 管工具，A2A 管 Agent 通信。
**AgentGuard 差距**: a2a_router 有内部路由但未遵循 Google A2A 标准，无法与外部系统互操作。
**实现方案**: 实现标准 Agent Card，暴露 A2A 端点（tasks/send, tasks/sendSubscribe），实现 TaskState 状态机。
**难度**: ⭐⭐⭐⭐ 较高 (12h) | **收益**: 🔵 跨生态互操作

---

## 实施路线图

```
Sprint 15-16 (生产加固):
  ├── 创新点 1: 节点级错误处理 + 崩溃恢复 (12h)
  └── 创新点 2: 会话历史压缩 (8h)

Sprint 17-18 (编排增强):
  ├── 创新点 3: HITL 审批门禁 (8h)
  ├── 创新点 6: 可调用条件边 (8h)
  └── 创新点 4: Agent-as-Tool (10h)

Sprint 19-20 (体验与可观测):
  ├── 创新点 8: 流式转换器 (10h)
  ├── 创新点 5: OTel GenAI 追踪 (10h)
  ├── 创新点 7: 会话缓冲服务 (8h)
  └── 创新点 9: Flow 持久化 (10h)

Sprint 21+ (生态互通):
  └── 创新点 10: A2A 协议支持 (12h)
```

**总工作量**: ~106h
**已实现相关功能**: MCP 协议(#22), GraphRAG(#25), 事件驱动(#27), A2A 路由(#17)

---

## 数据来源

| 框架 | 版本 | 仓库 |
|------|------|------|
| LangGraph | v1.2.0 (2026-05-12) | github.com/langchain-ai/langgraph |
| CrewAI | v1.14.5 (2026-05-15) | github.com/crewAIInc/crewAI |
| AutoGen | v0.7.5 (2025-09-30) | github.com/microsoft/autogen |
| OpenAI Agents SDK | v0.17.2 (2026-05-12) | github.com/openai/openai-agents-python |
| Google ADK | v1.33.0 (2026-05-08) | github.com/google/adk-python |
| Semantic Kernel | v1.42.0 (2026-05-14) | github.com/microsoft/semantic-kernel |

> 注：因无 web_search 工具可用，本摘要基于项目已有调研文档 (innovation-roadmap.md, github-agent-innovations-2026.md) 整理。框架版本信息来源于项目文档记录，建议通过各框架 GitHub release notes 交叉验证。
