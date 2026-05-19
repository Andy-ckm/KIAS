# Cognifold: Always-On Proactive Memory via Cognitive Folding

> 论文: 2605.13438 | 作者: Suli Wang, Yiqun Duan, Yu Deng, Rundong Zhao, Dai Shi, Xinliang Zhou
> 日期: 2026-05-13 | 分类: cs.AI, cs.CL
> 来源: https://arxiv.org/abs/2605.13438
> PDF: docs/papers/2605.13438.pdf (734KB)

## 摘要

现有 Agent 记忆系统以被动检索为主，缺乏自主组织经验为持久认知结构的能力。Cognifold 是一种脑启发的"始终在线"Agent 记忆系统，面向下一代主动助手设计。它持续将碎片化事件流折叠为自涌现的认知结构，从输入事件和累积知识中逐步引导更高层次认知。

**核心创新**：将互补学习系统（CLS）理论扩展到 Agent 记忆架构，实现"认知折叠"——类似大脑在海马体和新皮层之间巩固记忆的过程。

## 对 AgentGuard 的映射价值

### 1. 记忆层架构 (crates/knowledge/src/memory_layers.rs)
- **三层记忆系统**：AgentGuard 已有 WorkingMemory → EpisodicMemory → SemanticMemory
- **Cognifold 启示**：可增加"主动折叠"机制，让 Agent 在空闲时自动整理经验
- **实现方向**：在 memory_layers.rs 中增加 `CognitiveFolder` 异步任务

### 2. 知识图谱自动构建 (crates/knowledge/src/graph.rs)
- Cognifold 的"自涌现认知结构"与 AgentGuard KnowledgeGraph 的自动实体提取一致
- 可借鉴其"渐进式抽象"策略优化 entity_extractor

### 3. Agent 自主性 (crates/autonomy-controller/)
- 主动记忆是 FullAuto 模式的关键基础设施
- 与 AutonomyController 的三模式梯度天然契合

## 技术细节

- **认知折叠**：将事件流 → 情节记忆 → 语义知识的自动转化
- **始终在线**：不依赖查询触发，持续后台处理
- **渐进抽象**：从低级事件逐步构建高级概念

## AgentGuard 行动项

1. **[高]** 在 memory_layers.rs 中设计 CognitiveFolder trait
2. **[中]** 研究 CLS 理论如何映射到三层记忆的转换策略
3. **[低]** 评估异步折叠任务对 Agent 响应延迟的影响
