# Harnessing Agentic Evolution

> 论文: 2605.13821 | 作者: Jiayi Zhang, Yongfeng Gu
> 日期: 2026-05-13 | 分类: cs.AI
> 来源: https://arxiv.org/abs/2605.13821
> PDF: docs/papers/2605.13821.pdf (230KB)

## 摘要

Agent 演化已成为通过迭代生成候选方案、评估并用反馈指导未来搜索来改进程序、工作流和科学解决方案的强大范式。但现有方法通常是固定手工设计的流程（模块化但僵化），或通用 Agent（灵活整合反馈但在长期演化中容易漂移）。两种形式都积累了丰富的证据（候选方案、反馈、轨迹、失败），但缺乏稳定的接口来组织这些证据并重新利用。

## 对 AgentGuard 的映射价值

### 1. 工作流优化 (crates/workflow-engine/)
- **直接相关**：AgentGuard WorkflowEngine 执行 DAG 工作流
- **Agent 演化**：可让工作流在执行中自我优化
- **实现方向**：在 engine.rs 中增加演化反馈循环

### 2. 目标驱动循环 (crates/goal-engine/)
- GoalEngine 已有"自动迭代直到达标"机制
- Agent 演化提供了更系统的迭代改进框架
- **融合**：将演化策略整合到 loop_runner.rs

### 3. 自循环开发 (crates/auto-loop/)
- auto-loop 引擎专注于代码自动生成
- Agentic Evolution 为其提供了理论基础和优化策略
- 可借鉴其"证据组织"机制改进代码演化质量

### 4. 技能学习 (crates/skills/)
- Agent 演化过程中积累的经验可转化为新技能
- WebRecorder 已实现"操作录制→技能生成"
- 可扩展为"演化轨迹→技能提取"

## 技术细节

- **核心问题**：固定流程 vs 通用 Agent 的 trade-off
- **解决方案**：稳定接口组织演化证据
- **关键概念**：候选管理、反馈整合、轨迹复用

## AgentGuard 行动项

1. **[高]** 在 workflow-engine 中设计演化反馈接口
2. **[中]** 研究 goal-engine 与演化策略的融合方案
3. **[中]** 设计"演化证据"数据结构用于技能提取
4. **[低]** 评估演化循环对工作流执行时间的影响
