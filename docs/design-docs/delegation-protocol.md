# Agent 协作协议 - CrewAI 委托代理

> 设计文档 | KIAS Team Engine

## 1. 概述

本模块实现了 CrewAI 风格的多 Agent 协作协议，核心创新是**自主委托（Autonomous Delegation）**：

- Agent 可以自主判断自己是否适合执行当前任务
- 如果发现能力不匹配，自动委托给更合适的 Agent
- 委托决策基于技能匹配、负载均衡、历史成功率

## 2. 架构

```
┌─────────────────────────────────────────────┐
│                  Crew                        │
│  ┌─────────────┐  ┌───────────────────┐     │
│  │ ProcessMode  │  │ MemoryManager     │     │
│  │ Sequential   │  │ ┌─ShortTerm────┐  │     │
│  │ Hierarchical │  │ ├─LongTerm─────┤  │     │
│  └─────────────┘  │ └─Entity───────┘  │     │
│                    └───────────────────┘     │
│  ┌─────────────────────────────────────┐     │
│  │ DelegationProtocol                   │     │
│  │ ┌─DelegateRequest──┐                │     │
│  │ ├─DelegateResponse─┤ (Accept/Reject │     │
│  │ ├─ProgressUpdate───┤  CounterPropose│     │
│  │ └─DelegationResult─┘                │     │
│  └─────────────────────────────────────┘     │
│  ┌─────────────────────────────────────┐     │
│  │ SkillMatcher                         │     │
│  │ score = capability*0.6 + avail*0.2  │     │
│  │        + (1-load)*0.15 + success*0.05│    │
│  └─────────────────────────────────────┘     │
└─────────────────────────────────────────────┘
```

## 3. 模块说明

### 3.1 delegation.rs — 委托协议

**协议流程**：
```
Agent A (委托方)         Agent B (受托方)
     │                        │
     │──── DelegateRequest ──▶│
     │                        │── 评估能力
     │◀─── DelegateResponse ──│
     │    (Accept/Reject/     │
     │     CounterPropose)    │
     │                        │
     │──── TaskPayload ──────▶│
     │                        │── 执行
     │◀─── TaskResult ────────│
```

**状态机**：
```
Pending → Accepted → InProgress → Completed
    │         │           │
    └─ Rejected  └─ Cancelled
    └─ CounterProposed
    └─ TimedOut
    └─ Failed
```

### 3.2 memory.rs — Agent 记忆系统

| 层级 | 用途 | 淘汰策略 |
|------|------|----------|
| ShortTerm | 当前任务上下文 | TTL + LRU |
| LongTerm | 跨任务知识 | 访问频率 |
| Entity | 实体事实（Agent 能力等）| 置信度排序 |

### 3.3 skill_matcher.rs — 技能匹配器

**评分算法**：
```
score = capability_match × 0.6
      + availability × 0.2
      + (1.0 - load) × 0.15
      + historical_success × 0.05
```

支持：
- 按能力加权匹配（proficiency 0.0 - 1.0）
- 部分匹配与完全匹配模式
- 最低分数阈值过滤

### 3.4 crew.rs — Crew 编排器

**执行模式**：
- `Sequential`：任务顺序执行，前序输出作为后续上下文
- `Hierarchical`：层级委托，管理 Agent 分发给工作 Agent

**自主委托流程**：
1. SkillMatcher 选择最佳 Agent
2. 检查该 Agent 是否具备所有必需能力
3. 如不具备，查找更合适的 Agent 进行委托
4. 记录委托历史，供后续优化参考

## 4. 测试覆盖

| 模块 | 测试数 | 覆盖内容 |
|------|--------|----------|
| delegation | 10 | 状态机、消息变体、边界条件 |
| memory | 18 | 三层存储、淘汰、搜索、上下文构建 |
| skill_matcher | 11 | 评分、过滤、排序、权重配置 |
| crew | 18 | 执行模式、委托决策、共享记忆、错误处理 |
| **合计** | **57** | |

## 5. 使用示例

```rust
use kias_team_engine::crew::*;
use kias_team_engine::skill_matcher::*;

// 1. 创建 Crew
let crew = Crew::new("my-crew", CrewConfig {
    process_mode: ProcessMode::Hierarchical,
    enable_autonomous_delegation: true,
    ..CrewConfig::default()
});

// 2. 注册 Agent
crew.register_agent(CrewAgent {
    profile: AgentProfile::new("coder", "Code Expert")
        .with_capability("code_generation", 0.95)
        .with_capability("testing", 0.7),
    can_delegate: true,
    delegation_depth: 3,
}).await;

// 3. 定义任务
let tasks = vec![CrewTask {
    name: "write-tests".to_string(),
    description: "Write unit tests for the new module".to_string(),
    required_capabilities: vec!["testing".to_string()],
    expected_output: Some("A comprehensive test suite".to_string()),
    context: serde_json::json!({"module": "crew.rs"}),
    assigned_agent: None, // auto-select
}];

// 4. 执行
let result = crew.execute(tasks, &executor).await?;
```

## 6. 与现有系统集成

| 组件 | 集成方式 |
|------|----------|
| TeamEngine | Crew 可作为 TeamEngine 的后端执行器 |
| SwarmOrchestrator | Swarm 策略可调用 Crew 进行子任务编排 |
| Controller | Agent 心跳状态更新 EntityMemory |
| Scheduler | 调度器可使用 SkillMatcher 进行 Agent 选择 |
| WebSocket | 委托事件通过 EventBus 推送到 Dashboard |
