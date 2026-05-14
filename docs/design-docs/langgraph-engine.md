# LangGraph 状态图引擎设计文档

## 参考实现
- **LangGraph** (Python): 有向图状态机，支持条件分支、循环、中断/恢复
- **Temporal**: 工作流持久化、重试策略
- **XState**: 状态图理论基础（有限状态机 + 状态图）

## 核心概念

### 1. 状态图 (StateGraph)
- 有向图，节点是状态，边是转换
- 支持条件边（基于状态判断走哪条路）
- 支持循环（agent 自我迭代直到满足条件）
- **实现**: `WorkflowGraph` (`crates/workflow-engine/src/graph.rs`)

### 2. 类型化状态通道 (Typed Channels) ✅
- 每个通道有编译期类型 + reducer 策略
- 支持 `Replace`、`Append`、`Merge`、`Sum`、`KeepFirst` 内置 reducer
- 支持自定义 reducer（实现 `ChannelReducer<T>` trait）
- **实现**: `TypedState` (`crates/workflow-engine/src/typed_state.rs`)
- **创新点**: 类型擦除 + `Any` downcast 实现运行时类型安全，无需 macro

### 3. 流式执行 (Streaming) ✅
- `StreamingEvent` 枚举覆盖所有执行阶段：
  - `WorkflowStarted` / `WorkflowComplete` / `WorkflowFailed`
  - `NodeStart` / `NodeComplete`
  - `ChannelUpdate` / `EdgeTraversed` / `HumanInterrupt`
- `EventSink` 线程安全事件收集器，支持 `emit` / `take` / `peek`
- **实现**: `crates/workflow-engine/src/typed_state.rs`
- **创新点**: 非侵入式事件注入（`with_event_sink()`），零开销 when 无订阅者

### 4. 中断/恢复 (Interrupt/Resume) ✅
- `HumanReview` 节点类型 → 暂停执行，设置 `WaitingForHuman` 状态
- `CheckpointStore` 基于 `DashMap` 的并发安全检查点存储
- 支持从任意检查点恢复（`restore_from_checkpoint`）
- **实现**: `crates/workflow-engine/src/checkpoint.rs` + `engine.rs`

### 5. 子图组合 (Subgraph Composition) ✅
- `SubGraph` 封装子工作流图 + 输入/输出映射
- `input_mapping`: 父状态字段 → 子状态字段
- `output_mapping`: 子状态字段 → 父状态字段
- 支持超时控制、失败传播策略
- **实现**: `crates/workflow-engine/src/subgraph.rs`
- **创新点**: 隔离子引擎（无递归子图），Box::pin 避免无限 future 大小

## 数据模型

```rust
// 类型化状态通道
struct TypedState {
    channels: HashMap<String, ErasedChannel>,
    revision: u64,
}

// Reducer trait
trait ChannelReducer<T>: Send + Sync + 'static {
    fn reduce(&self, current: T, incoming: T) -> T;
    fn name(&self) -> &str;
}

// 子图组合
struct SubGraph {
    graph: WorkflowGraph,
    input_mapping: HashMap<String, String>,
    output_mapping: HashMap<String, String>,
    propagate_failure: bool,
    timeout_secs: Option<u64>,
}

// 流式事件
enum StreamingEvent {
    WorkflowStarted { workflow_id, entry_node, timestamp },
    NodeStart { workflow_id, node_id, node_type, revision, timestamp },
    NodeComplete { workflow_id, node_id, success, duration_ms, revision, timestamp },
    // ... more events
}
```

## 实现计划

### Phase 1: 基础图结构 ✅
- [x] StateGraph 构建器 (`WorkflowGraph`)
- [x] 节点注册 (`Node`, `NodeType`)
- [x] 边定义（普通 + 条件）(`Edge`, `Condition`)

### Phase 2: 执行引擎 ✅
- [x] 状态遍历 (`WorkflowEngine::execute`)
- [x] 条件分支 (`evaluate_condition`)
- [x] 循环支持（安全边界 = nodes * 100）

### Phase 3: 流式 + 中断 ✅
- [x] 流式输出 (`StreamingEvent` + `EventSink`)
- [x] 检查点持久化 (`CheckpointStore`)
- [x] 中断/恢复 (`HumanReview` + `restore_from_checkpoint`)

### Phase 4: 子图 ✅
- [x] 子图嵌套 (`SubGraph` + `SubWorkflow` 节点)
- [x] 递归执行（Box::pin 递归）

## 测试覆盖

| 模块 | 测试数 | 覆盖内容 |
|------|--------|----------|
| typed_state | 17 | reducer 策略、状态操作、事件 sink |
| engine | 28 | 线性/条件/循环/失败/重试/子图/流式事件 |
| checkpoint | 3 | 存储/查询/多版本 |
| graph | 5 | 构建/验证/拓扑 |
| subgraph | 5 | 映射/提取/合并/错误 |
| **总计** | **74** | |

## 验收标准
- [x] 编译通过
- [x] 测试覆盖所有核心功能 (74 tests)
- [x] 零 clippy 警告
- [x] 文档完整
- [x] 分层架构通过 (`make lint-arch`)
