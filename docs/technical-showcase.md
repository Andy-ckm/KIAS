# AgentGuard 技术深度剖析：AI Agent 集群调度系统

> **Kubernetes-like Intelligent Agent Scheduling System**  
> Rust 实现 · 21 个 Crate · 75,000+ 行代码 · 完整的 Agent 编排栈

---

## 目录

1. [系统全景](#1-系统全景)
2. [技术创新 #1：Cache-Aware 调度 — 将 KV Cache 命中率转化为调度决策](#2-cache-aware-调度)
3. [技术创新 #2：LangGraph 状态图引擎 — 并行扇出与检查点持久化](#3-langgraph-状态图引擎)
4. [技术创新 #3：TypedState 类型化状态通道 — 编译期安全的 Reducer 机制](#4-typedstate-类型化状态通道)
5. [技术创新 #4：三层记忆系统 — Agent 协作的持久化认知架构](#5-三层记忆系统)
6. [技术创新 #5：Worker-Verifier 对抗式质量门禁](#6-worker-verifier-对抗式质量门禁)
7. [技术创新 #6：自主度梯度控制器 — 分级执行权限与自动升级](#7-自主度梯度控制器)
8. [技术创新 #7：目标驱动循环引擎 — 训练循环自动化的系统化抽象](#8-目标驱动循环引擎)
9. [技术创新 #8：反调度器 — PDB 约束下的集群再平衡](#9-反调度器)
10. [技术创新 #9：A2A 协议 + MCP 沙箱 — Agent 互联与安全执行](#10-a2a-协议与-mcp-沙箱)
11. [技术创新 #10：数据脱敏框架 — 零信任日志安全](#11-数据脱敏框架)
12. [架构总结](#12-架构总结)

---

## 1. 系统全景

AgentGuard 不是一个简单的任务队列。它是一个借鉴 Kubernetes 控制平面架构、DeepSeek 缓存优化和 ANOLISA 可观测性理念构建的 **AI Agent 集群调度系统**。

```
┌─────────────────────────────────────────────────────────────────┐
│                         API Server (axum)                        │
│                   REST + gRPC + WebSocket + mTLS                 │
├─────────┬──────────┬──────────┬──────────┬───────────────────────┤
│Scheduler│Controller│Workflow  │  Team    │  Goal    │ Autonomy   │
│  Engine │          │  Engine  │  Engine  │  Engine  │ Controller │
│ 7算法   │ 心跳/恢复 │ DAG执行  │ OVW机制  │ 目标循环  │ 3级权限    │
├─────────┴──────────┴──────────┴──────────┴──────────┴───────────┤
│  LangGraph Engine    │   MCP Protocol    │   Data Store (SQLite) │
│  状态图+扇出+检查点   │   JSON-RPC+沙箱    │   向量+前缀缓存       │
├──────────────────────┴───────────────────┴───────────────────────┤
│                    Common (L0): 错误/配置/类型/A2A/脱敏           │
└─────────────────────────────────────────────────────────────────┘
```

**核心数字**：

| 指标 | 数值 |
|------|------|
| Rust Crate 数 | 21 |
| 代码行数 | 75,716 |
| 调度算法 | 7 种（含 GPU-Aware、Edge） |
| MCP 沙箱后端 | 5 种（Docker/Firecracker/gVisor/Wasm/Process） |
| 测试覆盖 | 每个 crate 均含 `#[cfg(test)]` 模块 |

---

## 2. Cache-Aware 调度

**文件**：`crates/scheduler/src/algorithms/cache_aware.rs`

### 问题

传统调度器（Round Robin、Least Loaded）对 LLM 推理场景是盲的：它们不知道某个节点上是否已经缓存了特定 system prompt 的 KV Cache。一次 cache miss 意味着重新计算整个 prefix，浪费约 90% 的 GPU 算力。

### 实现

AgentGuard 的 CacheAwareScheduler 将 **DeepSeek 风格的 Prefix Caching** 引入调度决策层：

```rust
// crates/scheduler/src/algorithms/cache_aware.rs:94-114
fn cache_aware_score(
    node: &Node,
    agent: &Agent,
    cache_info: Option<&NodeCacheInfo>,
    cache_weight: f64,
) -> f64 {
    let cache_score =
        if let (Some(info), Some(prefix_hash)) = (cache_info, agent.system_prompt_hash) {
            if info.cached_prefixes.contains(&prefix_hash) { 1.0 } else { 0.0 }
        } else { 0.0 };

    let load_score = 1.0 - node.load_factor();
    cache_weight * cache_score + (1.0 - cache_weight) * load_score
}
```

**关键设计**：

1. **快速路径**（第 132-153 行）：如果某节点有匹配的 cached prefix，直接路由，score = 1.0
2. **加权评分**（第 94-114 行）：无命中时，`score = cache_weight × cache_score + (1 - cache_weight) × load_score`
3. **DashMap 并发安全**：cache_map 使用 `Arc<DashMap>` 实现无锁并发读写

### 为什么重要

这是 **调度器层面唯一将 LLM 推理特性纳入决策的方案**。传统 K8S 调度器只看 CPU/Memory/GPU，不看 KV Cache 状态。AgentGuard 的 `cache_weight` 参数（0.0 = 纯负载均衡，1.0 = 纯缓存优先）让运维人员可以按业务特征调节。

---

## 3. LangGraph 状态图引擎

**文件**：`crates/langgraph-engine/src/graph.rs`

### 问题

LLM 应用的工作流不是线性的：有条件分支、循环重试、并行子任务、中断恢复。现有的 DAG 引擎（Airflow、Temporal）要么太重，要么不支持 LLM 特有的中断-恢复语义。

### 实现

AgentGuard 实现了一个完整的 LangGraph 风格状态图引擎，支持四种边类型：

```rust
// crates/langgraph-engine/src/graph.rs:38-55
pub enum EdgeType {
    Direct { from: String, to: String },
    Conditional { from: String, to: String, condition: EdgeCondition },
    Router { from: String, router: RouterFn },
    FanOut { from: String, targets: Vec<String>, join_node: String },
}
```

**并行扇出执行**（第 614-679 行）是核心亮点：

```rust
// crates/langgraph-engine/src/graph.rs:637-641
handles.push(tokio::spawn(async move {
    let result = (handler)(branch_state).await;
    (target, result)
}));
```

每个分支在独立的 tokio task 中并发执行，完成后的 state 变更通过 `merge` 合并（last-write-wins 策略）。

**检查点持久化**支持中断-恢复语义：

```rust
// crates/langgraph-engine/src/graph.rs:389-415
pub async fn resume_from_checkpoint(&self, checkpoint_id: &str) -> KiasResult<GraphState> {
    let checkpoint = self.checkpoint_store.load_by_id(checkpoint_id).await?
        .ok_or_else(|| /* ... */)?;
    let mut state = checkpoint.state.clone();
    state.metadata.is_interrupted = false;
    // 从 checkpoint 节点恢复，而非从 entry 重新开始
    self.execute_from(&checkpoint.node, state).await
}
```

### 为什么重要

1. **类型安全的 Handler**：`NodeHandler = Arc<dyn Fn(GraphState) -> Pin<Box<dyn Future<...>>>>` — 每个节点是强类型的 async 函数
2. **构建时验证**：`build()` 方法执行拓扑验证（检测不可达节点、缺失入口等），而非运行时才发现
3. **`max_steps` 防护**：防止死循环，这在 LLM 驱动的条件循环中至关重要

---

## 4. TypedState 类型化状态通道

**文件**：`crates/workflow-engine/src/typed_state.rs`

### 问题

LangGraph 的核心抽象是 TypedDict + Reducer 模式。在 Python 中，这依赖于类型提示（运行时检查）。在 Rust 中，可以用类型系统在编译期保证状态合并的正确性。

### 实现

```rust
// crates/workflow-engine/src/typed_state.rs:41-47
pub trait ChannelReducer<T>: Send + Sync + 'static {
    fn reduce(&self, current: T, incoming: T) -> T;
    fn name(&self) -> &str;
}
```

内置 5 种 Reducer：`Replace`、`Append`、`Merge`（HashMap 浅合并）、`KeepFirst`、`Sum`。

**类型擦除存储**（第 143-197 行）是工程亮点：

```rust
// crates/workflow-engine/src/typed_state.rs:162-190
impl ErasedChannel {
    fn new<T, R>(value: T, reducer: R) -> Self {
        let reduce_fn = Arc::new(
            move |current: Box<dyn Any + Send + Sync>,
                  incoming: Box<dyn Any + Send + Sync>| {
                let current = current.downcast::<T>().expect("type mismatch");
                let incoming = incoming.downcast::<T>().expect("type mismatch");
                let result = reducer_for_closure.reduce(*current, *incoming);
                Box::new(result)
            },
        );
        // ...
    }
}
```

每个 channel 将类型信息擦除为 `Box<dyn Any>`，但通过闭包捕获了原始类型 `T` 的 reducer。当 `update()` 调用时，通过 `downcast` 恢复类型安全性。

### 为什么重要

- **编译期安全**：如果 `T` 和 Reducer 不匹配，编译器会拒绝
- **运行时灵活**：channel 名称是字符串，支持动态注册
- **并发分支安全**：`FanOut` 执行后，各分支的 state 通过 reducer 确定性合并，无竞态

---

## 5. 三层记忆系统

**文件**：`crates/team-engine/src/memory.rs`

### 问题

多 Agent 协作的核心瓶颈不是通信，而是 **记忆**。Agent 执行完一个任务后，上下文丢失了。下次遇到类似问题，又要从零开始。

### 实现

AgentGuard 实现了三层记忆架构：

```rust
// crates/team-engine/src/memory.rs:339-362
pub struct MemoryManager {
    pub short_term: Arc<RwLock<ShortTermMemory>>,   // TTL 驱逐，任务级
    pub long_term: Arc<RwLock<LongTermMemory>>,     // 无 TTL，跨任务持久
    pub entity: Arc<RwLock<EntityMemory>>,           // 实体事实图谱
}
```

| 层级 | 驱逐策略 | 用途 |
|------|----------|------|
| ShortTerm | TTL + LRU | 当前任务上下文 |
| LongTerm | access_count + recency | 跨任务知识积累 |
| Entity | confidence + recency | 实体属性记忆 |

**ShortTerm 的自动驱逐**（第 144-154 行）：

```rust
// crates/team-engine/src/memory.rs:144-154
fn evict(&mut self) {
    self.entries.retain(|e| !e.is_expired());
    if self.entries.len() > self.max_entries {
        self.entries.sort_by_key(|a| a.last_accessed);
        self.entries.drain(0..self.entries.len() - self.max_entries);
    }
}
```

**ContextBuilder**（第 371-397 行）根据 token 预算组装上下文：

```rust
// crates/team-engine/src/memory.rs:381-396
pub fn build_context(&self, entries: &[MemoryEntry]) -> String {
    let mut approx_tokens = 0;
    for entry in entries {
        let entry_tokens = entry_text.len() / 4; // ~4 chars/token
        if approx_tokens + entry_tokens > self.max_tokens_approx { break; }
        context.push_str(&entry_text);
        approx_tokens += entry_tokens;
    }
    context
}
```

### 为什么重要

- **线程安全**：所有层级通过 `Arc<RwLock<>>` 保护，支持多 Agent 并发读写
- **Entity Memory** 不只是 KV 存储 — 它记录 **置信度**，让 Agent 可以区分「确定知道」和「猜测」
- **ContextBuilder** 的 token 预算管理解决了 LLM context window 溢出的实际问题

---

## 6. Worker-Verifier 对抗式质量门禁

**文件**：`crates/team-engine/src/verifier.rs`

### 问题

单 Agent 的输出质量不可控。即使使用了 CoT（Chain of Thought），LLM 仍会生成错误代码、遗漏边界条件、产生幻觉。

### 实现

AgentGuard 借鉴 MiniMax 的设计，实现了 **Worker-Verifier 对抗机制**：

```rust
// crates/team-engine/src/verifier.rs:5-11
/// Worker 停止的条件是 Verifier 启动的原因，
/// Verifier 停止的条件是尽可能发现 Worker 的问题，
/// 发现的问题又成为 Worker 重新启动的原因。
```

Verifier 不只是「检查」— 它是一个完整的质量门禁系统：

```rust
// crates/team-engine/src/verifier.rs:30-46
pub enum VerificationRule {
    Contains(String),
    NotContains(String),
    MinLength(usize),
    MaxLength(usize),
    ValidJson,
    Pattern(String),
    ShellCheck(String),  // 执行 shell 命令验证！
}
```

`ShellCheck` 规则允许在验证阶段运行测试命令（如 `cargo test`、`python -m pytest`），将验证从「看起来对」提升到「跑起来对」。

### 为什么重要

- **反馈闭环**：Verifier 的 issues 直接成为 Worker 下一轮的输入，形成迭代改进
- **可组合的规则**：`RuleBasedVerifier` 支持链式规则，每条规则独立判定
- **Shell 验证**：这是少数支持在验证阶段运行实际命令的 Agent 框架

---

## 7. 自主度梯度控制器

**文件**：`crates/autonomy-controller/src/autonomy.rs`

### 问题

AI Agent 的自主权管理是一个被严重忽视的问题。全自主模式危险，全确认模式低效。需要一个中间层来精细化控制。

### 实现

AgentGuard 实现了 Codex CLI 风格的三模式自主度控制：

```rust
// crates/autonomy-controller/src/ladder.rs
pub enum AutonomyLevel {
    Suggest,    // 仅建议，不执行
    AutoEdit,   // 写操作自动执行，其他需确认
    FullAuto,   // 全自动，但受限于工具策略
}
```

**完整的决策管线**（第 216-311 行）：

```
工具策略检查 → 速率限制检查 → 执行预算检查 → 自主度级别判断 → 审计日志
```

```rust
// crates/autonomy-controller/src/autonomy.rs:271-298
let decision = match autonomy_level {
    AutonomyLevel::Suggest => ExecutionDecision::SuggestOnly { .. },
    AutonomyLevel::AutoEdit => {
        if self.is_write_operation(tool) {
            ExecutionDecision::AutoExecute { requires_sandbox: true }
        } else {
            ExecutionDecision::RequiresConfirmation { .. }
        }
    }
    AutonomyLevel::FullAuto => ExecutionDecision::AutoExecute { requires_sandbox: .. },
};
```

**自动升级机制**（第 117-142 行）：

```rust
// crates/autonomy-controller/src/autonomy.rs:138-141
pub fn record_success(&mut self) -> bool {
    self.success_count += 1;
    self.success_count >= self.success_threshold
}
```

当 Agent 连续成功执行达到阈值时，自动从 Suggest 升级到 AutoEdit，减少人工干预。

### 为什么重要

- **粒度控制**：可以对单个工具设置不同的自主级别
- **不可绕过**：即使在 FullAuto 模式下，`Forbidden` 策略仍然生效
- **完整审计**：每次执行决策都记录到审计日志，支持事后审查

---

## 8. 目标驱动循环引擎

**文件**：`crates/goal-engine/src/loop_runner.rs`

### 问题

LLM 应用中常见的模式是「执行-评估-反馈-再执行」。但大多数框架把这个循环写在应用层的 while 循环里，缺乏标准化、检查点、取消、可观测性。

### 实现

AgentGuard 将这个模式抽象为 `GoalLoopRunner`：

```rust
// crates/goal-engine/src/loop_runner.rs:106-108
/// 核心公式：model.fit() = /goal
/// 训练循环自动化：定义目标 → 定义验证标准 → 运行循环
```

**执行器-评估器分离**（Worker-Judge 分离）：

```rust
// crates/goal-engine/src/loop_runner.rs:11-20
#[async_trait::async_trait]
pub trait RoundExecutor: Send + Sync {
    async fn execute_round(
        &self, goal: &Goal, round: u32,
        previous_feedback: Option<&EvaluationResult>,
    ) -> KiasResult<String>;
}
```

每轮执行后，独立的 `GoalEvaluator` 评估结果，并将反馈传递给下一轮执行。

**检查点持久化**支持崩溃恢复：

```rust
// crates/goal-engine/src/loop_runner.rs:154-167
pub async fn resume(&self, goal: Goal, checkpoint: GoalCheckpoint) -> KiasResult<GoalState> {
    let mut state = checkpoint.state;
    let mut last_evaluation = checkpoint.last_evaluation;
    self.run_inner(goal, &mut state, &mut last_evaluation).await
}
```

### 为什么重要

- **取消令牌**：`GoalCancelToken` 基于 `AtomicBool`，支持从外部优雅终止
- **反馈链追踪**：`evaluation_history` 记录每轮评估结果，支持事后分析收敛过程
- **检查点回调**：`checkpoint_callback` 允许外部系统（如数据库）在每轮后持久化状态

---

## 9. 反调度器

**文件**：`crates/scheduler/src/descheduler/engine.rs`

### 问题

调度器只管「往哪放」，不管「该不该搬走」。随着时间推移，集群会出现：低利用率节点浪费资源、反亲和性约束被违反、同一 Agent 的副本集中在少数节点。

### 实现

AgentGuard 实现了 K8S Descheduler 风格的反调度器：

```rust
// crates/scheduler/src/descheduler/engine.rs:21-24
pub struct DeschedulerEngine {
    config: DeschedulerConfig,
    strategies: Vec<Arc<dyn DeschedulerStrategy>>,
}
```

三种内置策略：

| 策略 | 文件 | 作用 |
|------|------|------|
| `LowNodeUtilization` | `strategies/low_utilization.rs` | 检测低利用率节点，迁移 Agent |
| `DuplicateAgent` | `strategies/duplicates.rs` | 检测同一 Agent 副本过度集中 |
| `AntiAffinityViolation` | `strategies/anti_affinity.rs` | 检测并修复反亲和性违规 |

**PDB 约束**（Pod Disruption Budget）确保驱逐不会导致服务中断：

```rust
// crates/scheduler/src/descheduler/engine.rs:66-80
pub async fn run(&self, snapshot: &ClusterSnapshot) -> Result<EvictionPlan, KiasError> {
    let mut all_evictions: Vec<Eviction> = Vec::new();
    for strategy in &self.strategies {
        let proposals = strategy.propose_evictions(&snapshot.nodes, &snapshot.agents).await?;
        all_evictions.extend(proposals);
    }
    // 去重 + PDB 约束 + 最大驱逐数限制
    // ...
}
```

### 为什么重要

- **Dry Run 模式**：支持预览驱逐计划而不实际执行
- **可插拔策略**：通过 trait 实现，易于扩展自定义策略
- **PDB 约束**：保证集群在再平衡过程中保持可用性

---

## 10. A2A 协议与 MCP 沙箱

**文件**：`crates/common/src/a2a.rs`、`crates/mcp-protocol/src/sandbox.rs`

### 问题

Agent 互联需要标准化协议；Agent 执行外部工具需要安全隔离。

### A2A 协议实现

AgentGuard 实现了 Google A2A（Agent-to-Agent）协议的完整数据模型：

```rust
// crates/common/src/a2a.rs:15-39
pub struct AgentCard {
    pub id: String,
    pub capabilities: AgentCapabilities,
    pub skills: Vec<AgentSkill>,
    pub authentication: Option<AuthInfo>,
    // ...
}
```

**任务生命周期**遵循 A2A 规范：

```rust
// crates/common/src/a2a.rs:106-121
pub enum A2aTaskStatus {
    Submitted, Working, InputRequired,
    Completed, Failed, Cancelled, Rejected,
}
```

**Agent 交接**（Handoff）支持 6 种原因：

```rust
// crates/common/src/a2a.rs:254-267
pub enum HandoffReason {
    CapabilityGap, LoadBalancing, Specialization,
    ErrorRecovery, HumanDirected, CostOptimization,
}
```

### MCP 沙箱实现

MCP 协议支持 **5 种沙箱后端**：

```rust
// crates/mcp-protocol/src/lib.rs:105-111
pub use sandbox::{
    FirecrackerSandboxBackend,  // 轻量 VM
    GVisorSandboxBackend,       // 用户态内核
    ProcessSandboxBackend,      // 进程级隔离
    WasmSandboxBackend,         // WebAssembly
    DockerSandboxBackend,       // Docker 容器
};
```

**快照机制**支持状态恢复：

```rust
// crates/mcp-protocol/src/sandbox.rs:67-86
pub struct SandboxSnapshot {
    pub sandbox_id: String,
    pub isolation_level: IsolationLevel,  // Session/User/Global
    pub files: HashMap<String, Vec<u8>>,
    pub env: HashMap<String, String>,
    pub workdir: Option<PathBuf>,
}
```

### 为什么重要

- **协议标准**：A2A 是 Google 提出的 Agent 互操作标准，AgentGuard 是少数实现了完整数据模型的 Rust 项目
- **5 种隔离级别**：从轻量（Process）到强隔离（Firecracker VM），按安全需求选择
- **MCP 完整实现**：含 OAuth 2.0 认证、RBAC、熔断器、限流器、凭证管理、热重载

---

## 11. 数据脱敏框架

**文件**：`crates/common/src/data_mask.rs`

### 问题

LLM 系统日志中经常泄露敏感数据：IP 地址、邮箱、JWT Token。传统做法是事后脱敏或依赖日志框架插件，容易遗漏。

### 实现

AgentGuard 在基础设施层实现了 **零信任脱敏**：

```rust
// crates/common/src/data_mask.rs:84-97
pub fn redact_log_message(msg: &str) -> String {
    let mut result = msg.to_string();
    result = redact_emails(&result);    // 自动检测邮箱
    result = redact_ips(&result);       // 自动检测 IPv4
    result = redact_tokens(&result);    // 自动检测长 token（≥32 字符）
    result
}
```

**SensitiveData 包装器**在 Display 和 Serialize 时自动脱敏：

```rust
// crates/common/src/data_mask.rs:301-311
impl fmt::Display for SensitiveData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.masked())  // 永远不会泄露原始值
    }
}

impl Serialize for SensitiveData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.masked())  // JSON 序列化也脱敏
    }
}
```

**IPv4 检测**是手写的确定性状态机（第 163-196 行），而非正则表达式，避免了 ReDoS 风险。

### 为什么重要

- **零信任**：脱敏在最底层（common crate），所有上层组件自动继承
- **类型安全**：`SensitiveData::email()`、`::ip()`、`::token()` 提供语义化脱敏
- **手动解析**：IP 地址检测使用手写解析器而非正则，性能更高且无 ReDoS 风险

---

## 12. 架构总结

### 分层架构

```
L0: common          ← 所有 crate 依赖的基础类型、错误、配置
L1: data-store      ← SQLite 持久化层
L2: scheduler, controller, workflow-engine, team-engine, ...
L3: api-server, kias-main
```

**严格单向依赖**：`make lint-arch` 自动检查，禁止跨层依赖。

### 技术栈选择

| 层面 | 技术 | 理由 |
|------|------|------|
| 异步运行时 | tokio | Rust 异步生态标准 |
| Web 框架 | axum | 类型安全的中间件系统 |
| 并发 Map | DashMap | 无锁并发，适合高频读写 |
| 序列化 | serde | 零成本抽象 |
| 配置 | config crate | TOML/YAML/JSON + 环境变量覆盖 |
| 日志 | tracing | 结构化日志，支持 span |
| 错误处理 | thiserror + anyhow | 业务错误用 thiserror，内部错误用 anyhow |

### 设计原则

1. **编译期安全 > 运行时检查**：TypedState、ChannelReducer
2. **可观测性内建**：StreamingEvent、EventBus、审计日志
3. **渐进式复杂度**：SimpleExecutor → RoundExecutor，InMemoryCheckpointStore → SqliteCheckpointStore
4. **无外部依赖硬编码**：SQLite 作为默认存储，etcd 可选，无 Redis 依赖

---

*AgentGuard — 不是又一个 Agent 框架，而是 Agent 集群的基础设施。*
