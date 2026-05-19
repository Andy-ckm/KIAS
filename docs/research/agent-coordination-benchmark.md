# When Does Hierarchy Help? Benchmarking Agent Coordination in Industrial Scheduling

> 论文: 2605.13172 | 作者: Ziqi Wang, Yuhao Yang
> 日期: 2026-05-13 | 分类: cs.AI
> 来源: https://arxiv.org/abs/2605.13172
> PDF: docs/papers/2605.13172.pdf (537KB)

## 摘要

Agent 和多 Agent 系统在工具使用、推理和协作任务上表现出色。但现有基准主要评估弱耦合环境中的任务完成度，对共享、动态演化系统中的层级和耦合约束协调支持有限。这留下了一个重要问题：不同协调范式何时成功或失败？本文引入 DESBench（分布式事件驱动调度基准），用于评估层级事件驱动工业调度中的 Agent 协调能力。

## 对 AgentGuard 的映射价值（⚠️ 高度相关）

### 1. 调度算法基准 (crates/scheduler/)
- **直接相关**：AgentGuard 调度器已有 RoundRobin/LeastLoaded/ResourceAware/CacheAware 四种算法
- **DESBench 启示**：可作为 AgentGuard 调度器的标准化评估基准
- **行动**：将 DESBench 集成到 benchmarks/ 中作为调度质量评估

### 2. 层级协调范式
- AgentGuard 有 Controller → Scheduler → Agent 的层级结构
- 论文研究"何时需要层级"——这对 AgentGuard 架构决策有直接指导意义
- **关键发现**（预期）：复杂耦合约束下层级优于扁平

### 3. 事件驱动调度
- AgentGuard 使用 etcd watch 事件驱动状态同步
- DESBench 的事件驱动模型与 AgentGuard 架构天然契合
- 可用于验证 AgentGuard 的事件处理延迟和吞吐量

## 技术细节

- **DESBench**：分布式事件驱动调度基准
- **评估维度**：任务完成率、调度延迟、资源利用率、协调开销
- **协调范式**：扁平 vs 层级 vs 混合

## AgentGuard 行动项

1. **[高]** 下载 DESBench 基准数据集，集成到 crates/benchmarks/
2. **[高]** 用 DESBench 评估 AgentGuard 四种调度算法的性能
3. **[中]** 研究层级协调 vs 扁平协调在 AgentGuard 场景下的 trade-off
4. **[低]** 考虑实现混合协调模式作为调度策略选项
