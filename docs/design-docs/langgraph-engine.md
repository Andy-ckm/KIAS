# LangGraph 状态图引擎设计文档 v2

## 参考实现
- **LangGraph** (Python): 有向图状态机，支持条件分支、循环、中断/恢复
- **Temporal**: 工作流持久化、重试策略
- **XState**: 状态图理论基础（有限状态机 + 状态图）
- **Tokio**: 异步运行时，broadcast channel 用于流式事件

## 架构概览

```text
┌─────────────────────────────────────────────────────────────┐
│                    StateGraphBuilder                         │
│  add_node() → add_edge() → add_conditional_edge()           │
│  add_router() → add_fan_out() → with_checkpoint_store()     │
│  with_stream() → with_max_steps() → build()                 │
└──────────────────────────┬──────────────────────────────────┘
                           │ validate()
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                      StateGraph                              │
│  execute()  ──────────────────────────────────────────────┐  │
│  execute_from()  ───────────────────────────────────────┐ │  │
│  resume_from_checkpoint()  ───────────────────────────┐ │ │  │
│  resume_latest()                                     │ │ │  │
│                                                       ▼ ▼ ▼  │
│  ┌──────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │  NodeHandler  │  │  Edge Routing    │  │ Fan-Out Engine │ │
│  │  (Arc<dyn Fn>)│  │  Direct/Cond/    │  │  (tokio::spawn │ │
│  │              │  │  Router/FanOut    │  │   + join_all)  │ │
│  └──────────────┘  └──────────────────┘  └────────────────┘ │
│                                                             │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │  CheckpointStore     │  │  ExecutionStream             │ │
│  │  (trait + InMemory)  │  │  (broadcast::channel)        │ │
│  └──────────────────────┘  └──────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 核心概念

### 1. 状态图 (StateGraph) ✅
- 有向图，节点是处理函数，边是转换
- 支持条件边（基于状态判断走哪条路）
- 支持循环（agent 自我迭代直到满足条件）
- **实现**: `crates/langgraph-engine/src/graph.rs`

### 2. 类型化状态通道 (Typed State) ✅
- `GraphState` 基于 `serde_json::Value` 的类型化通道
- `get<T>()` / `set<T>()` 提供编译期类型安全的读写 API
- `get_required<T>()` 返回 `Result`，强制检查缺失通道
- `merge()` / `merge_keep_existing()` 支持两种合并策略
- `snapshot()` / `restore_from_snapshot()` 支持状态快照
- **实现**: `crates/langgraph-engine/src/state.rs`

### 3. 流式执行 (Streaming) ✅
- `ExecutionEvent` 枚举覆盖所有执行阶段：
  - `NodeStart` / `NodeComplete` / `NodeError`
  - `EdgeTaken` / `Interrupted` / `Completed` / `Failed`
  - `CheckpointSaved` / `Resumed`
  - `BranchStart` / `BranchComplete` (fan-out)
- `ExecutionStream` 基于 `tokio::sync::broadcast` 的多消费者事件流
- `EventCollector` 用于测试/调试的事件收集器
- **实现**: `crates/langgraph-engine/src/stream.rs`
- **创新点**: broadcast channel 实现零开销的多订阅者模式

### 4. 中断/恢复 (Interrupt/Resume) ✅
- 节点处理器设置 `state.metadata.is_interrupted = true` 触发中断
- `CheckpointStore` trait 支持可插拔存储后端
- `InMemoryCheckpointStore` 基于 `RwLock<HashMap>` 的并发安全实现
- `resume_from_checkpoint()` 从中断点恢复执行
- `resume_latest()` 从最新检查点恢复
- `execute_from()` 支持从任意节点开始执行
- **实现**: `crates/langgraph-engine/src/checkpoint.rs` + `graph.rs`
- **创新点**: trait 化的检查点存储，可对接 etcd/SQLite

### 5. 子图组合 (Subgraph Composition) ✅
- `SubgraphNode` 封装子图，`into_handler()` 转换为节点处理器
- 支持嵌套执行（子图继承父状态）
- **实现**: `crates/langgraph-engine/src/graph.rs`

### 6. 路由器函数 (Router) ✅ NEW
- `add_router(from, router_fn)` — 动态多目标分支
- 路由器函数接收 `&GraphState`，返回目标节点名称
- 支持基于状态的任意路由逻辑
- **创新点**: 比条件边更灵活，支持 N 路分支

### 7. 并行扇出 (Fan-Out/Fan-In) ✅ NEW
- `add_fan_out(from, targets, join_node)` — 并行执行多个分支
- 使用 `tokio::spawn` 真正并发执行
- 分支完成后合并状态（last-write-wins）
- **创新点**: 真正的并行执行，不是顺序模拟

### 8. 图验证 (Graph Validation) ✅ NEW
- `validation::validate()` 在构建时检查图结构完整性
- 检查项：入口节点存在、边端点有效、无不可达节点
- 支持 reachability hints（Router/FanOut 的可达性提示）
- `build()` 自动验证，`build_unchecked()` 跳过验证
- **实现**: `crates/langgraph-engine/src/validation.rs`

## 模块结构

```text
crates/langgraph-engine/src/
├── lib.rs           # 模块声明 + re-exports
├── state.rs         # GraphState, StateMetadata, GraphStateSnapshot
├── graph.rs         # StateGraph, StateGraphBuilder, EdgeType, NodeHandler
├── checkpoint.rs    # CheckpointStore trait, InMemoryCheckpointStore, Checkpoint
├── stream.rs        # ExecutionEvent, ExecutionStream, EventCollector
└── validation.rs    # validate(), GraphTopology, ValidationError
```

## 测试覆盖

| 模块 | 测试数 | 覆盖内容 |
|------|--------|----------|
| validation | 6 | 有效图、缺失入口、悬空边、不可达节点、无边、循环 |
| integration | 33 | 线性图、条件分支、循环、路由器、扇出、中断/恢复、流式事件、错误处理、状态操作、检查点存储 |
| **总计** | **39** | |

## 验收标准
- [x] 编译通过
- [x] 测试覆盖所有核心功能 (39 tests)
- [x] 零 clippy 警告
- [x] 文档完整
- [x] 分层架构通过 (`make lint-arch`)
