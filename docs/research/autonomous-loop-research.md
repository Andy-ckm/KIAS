# AgentGuard 自主循环机制研究报告

> 研究日期：2026-05-19
> 研究方法：竞品分析 + 源码分析

## 一、核心问题

用户问："你怎么让自己主动起来？"

答案：**自主循环机制** — Agent不只是响应用户指令，而是能自主规划、执行、学习、恢复。

## 二、竞品分析

### 2.1 Prax-Agent（294⭐）

**核心机制：**

1. **Agent Loop** — 核心循环
   - 发送消息给LLM
   - 解析工具调用
   - 执行工具
   - 反馈结果
   - 重复直到模型返回最终文本
   - 最多25轮迭代（防无限循环）

2. **Memory Middleware** — 记忆中间件
   - 从对话中自动提取持久知识
   - 包括：workContext（项目背景）、topOfMind（当前焦点）、facts（事实）、triples（三元组）
   - 支持纠正信号检测（多语言）
   - 问题-解决方案模式提取

3. **Planning** — 规划引擎
   - LLM驱动的任务分解
   - 依赖关系识别
   - 静态回退计划（三步走）

4. **Error Recovery** — 错误恢复
   - 错误追踪
   - 自动重试

5. **Governance** — 治理机制
   - 预算控制
   - 权限管理

**关键洞察：**
- Agent应该从对话中自动学习，积累知识
- Agent应该有规划能力，能够分解任务
- Agent应该有错误恢复机制
- Agent应该受到治理机制的控制

### 2.2 TencentDB Agent Memory

**核心机制：**

1. **Mermaid Task Canvas** — 任务画布
   - 结构化任务图
   - 人可读的Mermaid格式
   - 支持折叠和展开

2. **Context Offloading** — 上下文卸载
   - 4层渐进存储
   - Level 0: 完整原文（refs/*.md）
   - Level 1: 工具调用级摘要（offload.jsonl）
   - Level 2: 任务画布节点（*.mmd）
   - Level 3: 任务级索引（上下文）

3. **Token优化**
   - 61% Token降耗
   - 成功率提升

**关键洞察：**
- 任务应该有结构化地图，不是线性历史
- 信息应该分层存储，按需检索
- 人可读的格式很重要

## 三、AgentGuard 自主循环设计

### 3.1 核心循环

```rust
/// AgentGuard 自主循环
pub struct AutonomousLoop {
    /// 任务规划器
    planner: Planner,
    /// 记忆系统
    memory: MemorySystem,
    /// 错误恢复
    error_recovery: ErrorRecovery,
    /// 治理机制
    governance: Governance,
    /// 任务画布
    canvas: TaskCanvas,
}

impl AutonomousLoop {
    /// 运行自主循环
    pub async fn run(&mut self, task: &str) -> Result<()> {
        // 1. 规划
        let plan = self.planner.generate_plan(task).await?;
        
        // 2. 执行
        for todo in plan.todos() {
            // 检查依赖
            if !self.check_dependencies(todo) {
                continue;
            }
            
            // 执行任务
            match self.execute_todo(todo).await {
                Ok(result) => {
                    // 记录成功
                    self.memory.record_success(todo, &result);
                    self.canvas.update(todo, "completed");
                }
                Err(e) => {
                    // 错误恢复
                    self.error_recovery.handle(todo, e).await?;
                }
            }
            
            // 检查治理
            if self.governance.should_stop() {
                break;
            }
        }
        
        // 3. 学习
        self.memory.extract_knowledge().await?;
        
        Ok(())
    }
}
```

### 3.2 记忆系统

```rust
/// 记忆系统
pub struct MemorySystem {
    /// 事实库
    facts: Vec<Fact>,
    /// 三元组库
    triples: Vec<Triple>,
    /// 问题-解决方案库
    solutions: Vec<Solution>,
}

/// 事实
pub struct Fact {
    content: String,
    category: FactCategory,
    confidence: f64,
    source: String,
}

/// 事实类别
pub enum FactCategory {
    Preference,    // 偏好
    Knowledge,     // 知识
    Context,       // 上下文
    Behavior,      // 行为
    Goal,          // 目标
    Correction,    // 纠正
}

/// 三元组
pub struct Triple {
    subject: String,
    predicate: String,
    object: String,
}
```

### 3.3 任务画布

```rust
/// 任务画布
pub struct TaskCanvas {
    /// 任务节点
    nodes: Vec<TaskNode>,
    /// 依赖关系
    edges: Vec<Dependency>,
}

/// 任务节点
pub struct TaskNode {
    id: String,
    content: String,
    status: TaskStatus,
    started_at: DateTime,
    completed_at: Option<DateTime>,
}

/// 任务状态
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}
```

### 3.4 治理机制

```rust
/// 治理配置
pub struct Governance {
    /// 最大迭代次数
    max_iterations: usize,
    /// 最大Token消耗
    max_tokens: usize,
    /// 最大时间（秒）
    max_time: u64,
    /// 需要确认的操作
    require_confirmation: Vec<String>,
}

impl Governance {
    /// 检查是否应该停止
    pub fn should_stop(&self) -> bool {
        self.iterations >= self.max_iterations
            || self.tokens_used >= self.max_tokens
            || self.elapsed() >= self.max_time
    }
}
```

## 四、实施计划

### Phase 1：基础循环（本周）
- [ ] 实现Agent Loop
- [ ] 实现基础记忆系统
- [ ] 实现静态规划

### Phase 2：智能循环（下周）
- [ ] 实现LLM驱动的规划
- [ ] 实现错误恢复
- [ ] 实现治理机制

### Phase 3：任务画布（第三周）
- [ ] 实现Mermaid任务画布
- [ ] 实现上下文卸载
- [ ] 实现Token优化

## 五、关键结论

1. **自主循环 = 规划 + 执行 + 学习 + 恢复 + 治理**
2. **记忆系统是核心** — Agent应该从对话中自动学习
3. **任务画布很重要** — 结构化地图比线性历史更好
4. **治理机制不可少** — 防止无限循环和资源耗尽

## 六、参考源码

```
/mnt/reference-projects/
├── prax-agent/          # 自主循环参考
└── TencentDB-Agent-Memory/  # 任务画布参考
```
