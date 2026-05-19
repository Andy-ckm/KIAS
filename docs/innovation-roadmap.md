# AgentGuard 创新路线图 — 基于主流 AI Agent 框架最新特性研究

> **研究日期**: 2026-05-16
> **研究范围**: LangGraph v1.2, CrewAI v1.14, AutoGen v0.7, Semantic Kernel v1.42, OpenAI Agents SDK v0.17, Google ADK v1.33
> **筛选标准**: 生产可用 ✅ | AgentGuard 尚未实现 ✅ | Rust 可行 ✅

---

## 研究背景

对 6 大主流 AI Agent 框架的最新版本（截至 2026 年 5 月）进行了深度分析，包括 release notes、架构设计、新特性等。以下 10 个特性经过严格筛选，确保：
1. 在生产环境中有实际价值
2. AgentGuard 当前代码库中尚未实现
3. 使用 Rust 实现技术上可行

---

## 特性 1: 节点级错误处理器 + 宿主崩溃恢复

**来源框架**: LangGraph v1.2.0 (2026-05-12)
**相关 PR**: `#7773` (durable error-handler resume across host crashes), `#7233` (node-level error handlers)

### 技术原理
LangGraph 在 v1.2 中引入了两个关键能力：
- **节点级错误处理器**: 每个 StateGraph 节点可以注册独立的 `error_handler`，当节点执行失败时，由 handler 决定是重试、跳过、还是路由到降级路径，而非直接中断整个图执行。
- **宿主崩溃恢复 (Durable Resume)**: 当宿主进程崩溃后重启，通过 checkpoint 中的 writes history 自动恢复到崩溃前的执行状态，实现"断点续跑"。关键机制是 `get_writes_history` API + `DeltaChannel` 增量快照。

### AgentGuard 差距分析
AgentGuard 的 `workflow-engine` 有 checkpoint 持久化和重试机制，但：
- 错误处理是引擎级的，不是节点级的（无法为单个节点定义独立的错误恢复策略）
- 检查点是全量快照，没有增量持久化
- 没有跨宿主崩溃的自动恢复能力

### 实现方案
```
文件: crates/workflow-engine/src/node.rs (增强)
      crates/workflow-engine/src/error_handler.rs (新增)
      crates/workflow-engine/src/checkpoint.rs (增强)

核心改动:
1. 为 WorkflowNode 增加 ErrorHandler trait:
   trait ErrorHandler {
       async fn on_error(&self, ctx: &NodeContext, err: &KiasError) -> ErrorAction;
       // ErrorAction: Retry(max, backoff) | Skip | Fallback(node_id) | Abort
   }
2. 增量检查点: 将 checkpoint 从全量 JSON 快照改为 WAL (Write-Ahead Log) 模式
   - 每个节点执行完成后写入 delta record
   - 恢复时 replay delta records 到最新一致状态
3. 崩溃恢复: 在 engine 启动时扫描未完成的 checkpoint，自动 resume

预计工作量: 12h
收益: 生产环境可靠性大幅提升，长时间工作流不怕进程崩溃
```

---

## 特性 2: 流式转换器基础设施 (Streaming Transformer)

**来源框架**: LangGraph v1.2.0 (2026-05-12)
**相关 PR**: `#7519` (streaming transformer infrastructure), `#7677` (stream_events v3)

### 技术原理
LangGraph 引入了 `StreamingTransformer` 基础设施，允许在图执行过程中对流式输出进行实时转换和过滤：
- 支持 `stream_events(version='v3')` 协议，提供更精细的事件流（每个 superstep 的 token 级别输出）
- 用户可以注册 transformer 函数，在流式数据到达客户端之前进行过滤、聚合、格式化
- 支持多种投影模式：custom（自定义）、updates（节点更新）、checkpoints（检查点）、debug（调试）、tasks（任务）

### AgentGuard 差距分析
AgentGuard 的 workflow-engine 和 langgraph-engine 都是"执行完返回结果"模式，没有流式输出能力。对于 LLM 类任务，用户无法实时看到生成进度。

### 实现方案
```
文件: crates/workflow-engine/src/stream.rs (新增)
      crates/langgraph-engine/src/stream.rs (新增)

核心改动:
1. 定义 StreamEvent 枚举:
   enum StreamEvent {
       NodeStarted { node_id, timestamp },
       TokenEmitted { node_id, token, position },
       NodeCompleted { node_id, result },
       NodeError { node_id, error },
       CheckpointSaved { checkpoint_id },
   }
2. 实现 StreamTransformer trait:
   trait StreamTransformer {
       fn transform(&self, event: StreamEvent) -> Option<StreamEvent>;
   }
3. 在 engine 中集成 broadcast channel (tokio::sync::broadcast)
4. 通过 SSE/WebSocket 将事件流推送给客户端

预计工作量: 10h
收益: LLM 任务实时流式输出，用户体验质的飞跃
```

---

## 特性 3: Human-in-the-Loop (HITL) 审批门禁

**来源框架**: CrewAI v1.14.2 (2026-04), OpenAI Agents SDK v0.16
**相关功能**: HITL pre-review, approval_func, local approval rejection

### 技术原理
CrewAI 和 OpenAI Agents SDK 都引入了标准化的 HITL 机制：
- **CrewAI**: Flow 执行到关键节点时自动暂停，发送审批请求，支持"pre-review"（预审）和"distillation"（精炼确认）两种模式。HITL resume 后触发 `flow_finished` 事件。
- **OpenAI Agents SDK**: 通过 `approval_func` 回调，让外部系统决定是否批准工具调用。支持保留审批拒绝原因，用于审计。

### AgentGuard 差距分析
AgentGuard 的 workflow-engine 有 `HumanApproval` 节点类型，但是：
- 只支持简单的人工审批节点，没有 pre-review 模式
- 没有审批策略 DSL（无法定义自动审批规则）
- 没有审批历史和审计追踪

### 实现方案
```
文件: crates/workflow-engine/src/approval.rs (增强)
      crates/common/src/approval_policy.rs (新增)

核心改动:
1. 定义 ApprovalPolicy:
   enum ApprovalPolicy {
       Always,                          // 总是需要人工审批
       Threshold { risk_score: f64 },   // 风险评分超过阈值时审批
       AutoApprove { conditions },      // 满足条件自动通过
       HumanReview { timeout: Duration },// 超时自动拒绝
   }
2. 增加审批上下文传递:
   struct ApprovalContext {
       node_id, action, risk_score, preview_output, history
   }
3. 审批决策持久化到审计日志
4. 支持审批超时自动降级（降级到安全路径）

预计工作量: 8h
收益: 生产级人机协作能力，降低自动化风险
```

---

## 特性 4: GraphFlow 可调用条件边

**来源框架**: AutoGen v0.6.0 (2025-06)
**相关 PR**: `#6623` (callable condition for GraphFlow edges)

### 技术原理
AutoGen 在 v0.6 中引入了 GraphFlow 的可调用条件边：
- 之前的条件边只能基于关键字子串匹配（keyword substring），覆盖场景有限
- 新方案允许传入 lambda 函数或其他 callable 作为边条件
- 条件函数接收当前状态，返回布尔值决定是否走该边
- 支持 DAG 拓扑，多个 agent 并行或串行执行

### AgentGuard 差距分析
AgentGuard 的 langgraph-engine 有条件边（`ConditionalEdge`），但实现是基于简单字符串匹配或枚举值路由。没有支持自定义闭包/函数作为条件的机制。

### 实现方案
```
文件: crates/langgraph-engine/src/edge.rs (增强)
      crates/langgraph-engine/src/condition.rs (新增)

核心改动:
1. 定义 ConditionEvaluator trait:
   trait ConditionEvaluator: Send + Sync {
       fn evaluate(&self, state: &StateSnapshot) -> String; // 返回下一个节点 ID
   }
2. 内置条件类型:
   - RegexMatch { pattern, field }
   - JsonPath { path, expected }
   - NumericCompare { field, op, value }
   - CustomScript { script: String }  // 小型 DSL 或 Lua 脚本
3. 边定义支持 ConditionEvaluator:
   struct ConditionalEdge {
       from: NodeId,
       evaluator: Box<dyn ConditionEvaluator>,
       targets: HashMap<String, NodeId>, // 结果 -> 目标节点映射
   }

预计工作量: 8h
收益: 工作流路由逻辑极大增强，支持复杂业务规则
```

---

## 特性 5: Agent-as-Tool 模式

**来源框架**: AutoGen v0.7 (2025-08)
**相关功能**: Agent-as-Tool, TeamTool

### 技术原理
AutoGen 引入了 `Agent-as-Tool` 模式：
- 一个 Agent 可以被封装为工具，供其他 Agent 调用
- 调用方 Agent 通过标准工具调用接口触发被封装的 Agent
- 被封装 Agent 执行完成后，结果作为工具返回值传回调用方
- 支持 `TeamTool` 模式——将一个 Agent 团队封装为单个工具

### AgentGuard 差距分析
AgentGuard 的 team-engine 支持 Owner-Worker-Verifier 三角色，但：
- Agent 之间只能通过 team 内部协议通信
- 没有"将 Agent 注册为工具"的能力
- 无法实现 Agent 层级嵌套调用

### 实现方案
```
文件: crates/team-engine/src/agent_tool.rs (新增)
      crates/skills/src/registry.rs (增强)

核心改动:
1. 实现 AgentTool 包装器:
   struct AgentTool {
       agent_id: AgentId,
       description: String,    // 工具描述，用于 LLM 理解
       input_schema: Schema,   // 输入 JSON Schema
       timeout: Duration,
   }
   impl Tool for AgentTool {
       async fn execute(&self, input: Value) -> ToolResult {
           // 调度 agent 执行任务，等待完成，返回结果
       }
   }
2. 将 AgentTool 注册到 skill registry
3. 支持 Agent 团队作为工具: TeamTool { agents, orchestration_mode }

预计工作量: 10h
收益: Agent 能力可组合，实现层级化多 Agent 架构
```

---

## 特性 6: 会话历史压缩 (Session Compaction)

**来源框架**: OpenAI Agents SDK v0.16 (2026-05)
**相关功能**: Session history compaction, Conversations reasoning persistence

### 技术原理
OpenAI Agents SDK 引入了会话历史压缩机制：
- 当对话历史超过 token 限制时，自动压缩旧消息
- 保留最近的消息不变，将早期消息摘要化
- 压缩失败时自动恢复原始历史（`restore after compaction replacement failures`）
- 支持 reasoning 内容的持久化（保留推理过程用于审计）

### AgentGuard 差距分析
AgentGuard 的 team-engine 有三层记忆系统，但没有会话历史压缩能力。当 Agent 处理长对话时，容易超出 token 限制。

### 实现方案
```
文件: crates/team-engine/src/memory/compaction.rs (新增)

核心改动:
1. 定义 CompactionStrategy:
   trait CompactionStrategy {
       async fn compact(&self, messages: &[Message]) -> CompactedHistory;
   }
2. 实现两种策略:
   - SlidingWindow: 保留最近 N 条消息 + 系统 prompt
   - SummarizeOld: 用 LLM 将旧消息摘要为一条 summary message
3. 压缩前后快照备份（支持回滚）
4. Token 计数器集成:
   fn should_compact(messages: &[Message], max_tokens: usize) -> bool

预计工作量: 8h
收益: 长对话不再爆 token，降低 LLM 调用成本
```

---

## 特性 7: OpenTelemetry 原生 Agent 追踪

**来源框架**: AutoGen v0.6.2 (GenAI Semantic Convention), Google ADK v1.32 (native OTel metrics)
**相关功能**: GenAI Semantic Convention spans, native OpenTelemetry agentic metrics

### 技术原理
AutoGen 和 Google ADK 都采用了 OpenTelemetry 的 GenAI Semantic Convention：
- **AutoGen**: 遵循 `create_agent`、`invoke_agent`、`execute_tool` 三种标准 span
- **Google ADK**: 原生集成 OpenTelemetry metrics，暴露 agent 级别的度量指标
- 标准化的 trace 格式可以无缝对接 Jaeger、Grafana Tempo 等工具
- 支持环境变量控制是否启用 tracing

### AgentGuard 差距分析
AgentGuard 有 Prometheus 指标和基础的 tracing，但：
- 没有遵循 GenAI Semantic Convention 标准
- 没有 Agent 级别的 trace span（create/invoke/tool_call）
- trace 数据格式不标准化，无法与其他 Agent 框架互通

### 实现方案
```
文件: crates/common/src/tracing/genai_spans.rs (新增)
      crates/monitor/src/opentelemetry.rs (增强)

核心改动:
1. 实现 GenAI Semantic Convention span:
   - gen_ai.agent.create: Agent 创建
   - gen_ai.agent.invoke: Agent 调用（输入/输出 token 数）
   - gen_ai.tool.execute: 工具执行
   - gen_ai.workflow.step: 工作流步骤
2. 每个 span 包含标准属性:
   gen_ai.agent.name, gen_ai.agent.id, gen_ai.system,
   gen_ai.request.model, gen_ai.usage.input_tokens, gen_ai.usage.output_tokens
3. 集成 opentelemetry-rust SDK
4. 支持环境变量 AgentGuard_OTEL_ENABLED 控制开关

预计工作量: 10h
收益: 标准化可观测性，与 Jaeger/Grafana 无缝集成，跨框架互操作
```

---

## 特性 8: Bufferable Session Service (会话缓冲服务)

**来源框架**: Google ADK v1.33.0 (2026-05-08)
**相关功能**: BufferableSessionService

### 技术原理
Google ADK 引入了 `BufferableSessionService`：
- 将会话状态的读写缓冲在内存中，批量刷新到持久化存储
- 减少频繁的 I/O 操作（特别是 SQLite/etcd 写入）
- 支持 flush 策略：定时刷新、达到阈值刷新、手动刷新
- 崩溃时通过 WAL 恢复未刷新的数据

### AgentGuard 差距分析
AgentGuard 的 data-store 直接读写 SQLite，每次状态变更都会触发磁盘 I/O。在高并发场景下，这会成为性能瓶颈。

### 实现方案
```
文件: crates/data-store/src/buffer.rs (新增)

核心改动:
1. 实现 SessionBuffer:
   struct SessionBuffer {
       write_buffer: DashMap<SessionId, SessionState>,  // 并发安全的写缓冲
       flush_interval: Duration,
       max_buffer_size: usize,
   }
2. Flush 策略:
   - Timer-based: 每 N 秒刷新一次
   - Size-based: 缓冲区满时刷新
   - Manual: 显式调用 flush()
3. WAL 保障: 写入缓冲前先写 WAL，崩溃后从 WAL 恢复
4. 与现有 SQLite store 透明集成（实现相同 trait）

预计工作量: 8h
收益: 高并发场景下 I/O 性能提升 5-10 倍
```

---

## 特性 9: A2A (Agent-to-Agent) 协议支持

**来源框架**: CrewAI v1.14.2 (2026-04) + Google A2A Protocol
**相关功能**: Enterprise A2A, OSS A2A documentation

### 技术原理
Google 发布了 A2A (Agent-to-Agent) 协议标准，CrewAI 已率先实现：
- 每个 Agent 暴露一个标准化的 Agent Card（JSON 描述文件），声明能力、端点、认证方式
- Agent 之间通过 A2A 协议进行任务委托和结果返回
- 支持跨系统、跨框架的 Agent 互操作
- 与 MCP 互补：MCP 管理工具，A2A 管理 Agent 间通信

### AgentGuard 差距分析
AgentGuard 的 a2a_router 已实现了内部的 5 种路由策略，但：
- 没有遵循 Google A2A 协议标准
- Agent Card 格式不标准
- 无法与外部 A2A 兼容系统互操作

### 实现方案
```
文件: crates/api-server/src/a2a/ (增强)
      crates/common/src/agent_card.rs (新增)

核心改动:
1. 实现标准 Agent Card:
   struct AgentCard {
       name: String,
       description: String,
       url: String,
       capabilities: Vec<AgentCapability>,
       authentication: AuthConfig,
       default_input_modes: Vec<String>,
       default_output_modes: Vec<String>,
   }
2. 暴露 A2A 标准端点:
   - POST /a2a/v1/tasks/send       (同步任务)
   - POST /a2a/v1/tasks/sendSubscribe (SSE 流式)
   - GET  /a2a/v1/agents/{id}/card (获取 Agent Card)
3. 实现 A2A Task 对象模型 (TaskState: submitted/working/input-required/completed/failed)
4. 与现有 a2a_router 集成

预计工作量: 12h
收益: 跨系统 Agent 互操作，与 Google 生态、CrewAI 生态打通
```

---

## 特性 10: Flow 持久化与状态恢复 (Flow Persistence)

**来源框架**: CrewAI v1.14.4 (2026-04)
**相关 PR**: `#5649` (custom persistence key in @persist)

### 技术原理
CrewAI 为 Flow 引入了声明式持久化：
- 通过 `@persist` 装饰器标记需要持久化的状态字段
- 支持自定义 persistence key（不同 Flow 实例可以共享/隔离状态）
- Flow 执行到任意步骤都可以保存状态快照
- 恢复时从指定快照继续执行（`restoreFromStateId`）
- 通过 TUI/API 检查 Flow checkpoint 并恢复

### AgentGuard 差距分析
AgentGuard 的 workflow-engine 有 checkpoint 机制，但是：
- 没有"声明式"持久化（无法指定哪些状态需要持久化）
- 没有按 Flow 实例的快照管理和恢复
- 没有从 UI 层面查看和选择 checkpoint 的能力

### 实现方案
```
文件: crates/workflow-engine/src/persistence.rs (新增)
      crates/langgraph-engine/src/persistence.rs (增强)

核心改动:
1. 声明式持久化标记:
   #[derive(Persistable)]
   struct AgentState {
       #[persist(key = "conversation")]
       messages: Vec<Message>,
       #[persist(key = "context")]
       context: HashMap<String, Value>,
       #[persist(skip)]
       temp_data: Vec<u8>,  // 不持久化
   }
2. StateStore trait:
   trait StateStore {
       async fn save(&self, flow_id: &str, state_id: &str, snapshot: StateSnapshot);
       async fn load(&self, flow_id: &str, state_id: &str) -> Option<StateSnapshot>;
       async fn list_checkpoints(&self, flow_id: &str) -> Vec<CheckpointInfo>;
       async fn restore(&self, flow_id: &str, state_id: &str) -> Result<()>;
   }
3. 自动快照: 每个节点执行完成后自动保存（可配置频率）
4. API 端点: GET /api/v1/workflows/{id}/checkpoints, POST /.../restore

预计工作量: 10h
收益: 长时间工作流可随时回溯，调试和恢复能力大幅提升
```

---

## 优先级排序

| 优先级 | 特性 | 来源 | 工作量 | 收益 |
|--------|------|------|--------|------|
| **P0** | 1. 节点级错误处理 + 崩溃恢复 | LangGraph | 12h | 🔴 关键可靠性 |
| **P0** | 6. 会话历史压缩 | OpenAI SDK | 8h | 🔴 生产必须 |
| **P1** | 3. HITL 审批门禁 | CrewAI/OpenAI | 8h | 🟡 安全性 |
| **P1** | 2. 流式转换器 | LangGraph | 10h | 🟡 用户体验 |
| **P1** | 4. 可调用条件边 | AutoGen | 8h | 🟡 编排能力 |
| **P1** | 5. Agent-as-Tool | AutoGen | 10h | 🟡 组合能力 |
| **P2** | 7. OTel GenAI 追踪 | AutoGen/ADK | 10h | 🟢 可观测性 |
| **P2** | 8. 会话缓冲服务 | Google ADK | 8h | 🟢 性能优化 |
| **P2** | 10. Flow 持久化 | CrewAI | 10h | 🟢 可靠性 |
| **P3** | 9. A2A 协议 | CrewAI/Google | 12h | 🔵 互操作性 |

---

## 实施建议

### 第一阶段 (Sprint 15-16): 生产加固
- 特性 1 (节点级错误处理) + 特性 6 (会话压缩) = 20h
- 目标: 解决生产环境最紧迫的可靠性和成本问题

### 第二阶段 (Sprint 17-18): 编排增强
- 特性 3 (HITL) + 特性 4 (条件边) + 特性 5 (Agent-as-Tool) = 26h
- 目标: 提升 Agent 编排的灵活性和安全性

### 第三阶段 (Sprint 19-20): 体验与可观测
- 特性 2 (流式) + 特性 7 (OTel) + 特性 8 (缓冲) + 特性 10 (持久化) = 38h
- 目标: 提升用户体验和运维能力

### 第四阶段 (Sprint 21+): 生态互通
- 特性 9 (A2A 协议) = 12h
- 目标: 与外部 Agent 生态打通

---

## 参考资源

| 框架 | 版本 | 仓库 |
|------|------|------|
| LangGraph | v1.2.0 (2026-05-12) | github.com/langchain-ai/langgraph |
| CrewAI | v1.14.5 (2026-05-15) | github.com/crewAIInc/crewAI |
| AutoGen | v0.7.5 (2025-09-30) | github.com/microsoft/autogen |
| Semantic Kernel | v1.42.0 (2026-05-14) | github.com/microsoft/semantic-kernel |
| OpenAI Agents SDK | v0.17.2 (2026-05-12) | github.com/openai/openai-agents-python |
| Google ADK | v1.33.0 (2026-05-08) | github.com/google/adk-python |
