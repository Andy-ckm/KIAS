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

### 2. 类型化状态通道 (Typed Channels)
- 状态是类型化的结构体
- 每个节点可以读写特定通道
- 防止状态混乱

### 3. 流式执行 (Streaming)
- 节点执行时可以产生中间结果
- 支持 SSE/WebSocket 推送

### 4. 中断/恢复 (Interrupt/Resume)
- 可以在任意节点暂停
- 支持人工介入后恢复
- 检查点持久化

### 5. 子图组合 (Subgraph Composition)
- 一个节点可以是另一个状态图
- 支持递归组合

## 数据模型

```rust
// 状态通道
trait Channel: Send + Sync {
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

// 图节点
struct Node {
    name: String,
    handler: Box<dyn Fn(State) -> Pin<Box<dyn Future<Output = State>>>>,
}

// 条件边
struct Edge {
    from: String,
    to: String,
    condition: Option<Box<dyn Fn(&State) -> bool>>,
}

// 状态图
struct StateGraph {
    nodes: HashMap<String, Node>,
    edges: Vec<Edge>,
    entry: String,
    checkpoints: Vec<Checkpoint>,
}
```

## 实现计划

### Phase 1: 基础图结构
- [ ] StateGraph 构建器
- [ ] 节点注册
- [ ] 边定义（普通 + 条件）

### Phase 2: 执行引擎
- [ ] 状态遍历
- [ ] 条件分支
- [ ] 循环支持

### Phase 3: 流式 + 中断
- [ ] 流式输出
- [ ] 检查点持久化
- [ ] 中断/恢复

### Phase 4: 子图
- [ ] 子图嵌套
- [ ] 递归执行

## 验收标准
- [ ] 编译通过
- [ ] 测试覆盖所有核心功能
- [ ] 零 clippy 警告
- [ ] 文档完整
