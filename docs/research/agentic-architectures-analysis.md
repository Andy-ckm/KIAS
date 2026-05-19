# Agent 架构演化全景分析框架

> 来源: all-agentic-architectures 项目 (agno 实现)
> 日期: 2026-05-18
> 状态: AgentGuard 架构方法论参考

## 统一分析框架（六个固定问题）

每种架构用同一套问题拆解：
1. **它要解决什么问题？** 上一代架构哪里不够
2. **它的 State 是什么？** 新增了哪些字段，为什么必须存在
3. **它的拓扑是什么？** 线性链、循环、分叉汇聚、共享黑板、树搜索、网格涌现
4. **它的 Router 怎么工作？** 固定边、条件边、动态调度、验证回路、人工审批
5. **它的失败模式是什么？** 架构最容易在哪个环节坏掉
6. **什么时候该升级到下一种？** 当前模式的能力边界在哪里

## 演化路径：逐步添加控制能力

| # | 阶段 | 新增能力 | 核心解释 | 代表架构 | AgentGuard 对应 |
|---|------|---------|---------|---------|----------|
| 1 | 单次生成 | 基础 LLM 调用 | 输入→输出，无控制流 | Direct | — |
| 2 | Reflection | 生成+评估+修正 | generator + critic + refiner 三阶段 | Reflection | quality_pipeline ✅ |
| 3 | Tool Use | 结构化世界交互 | 文本→结构化→文本的跨越 | Function Calling | tool-executor + MCP ✅ |
| 4 | ReAct | 观察-行动循环 | Thought→Action→Observation 滚动 | ReAct | auto-loop ✅ |
| 5 | Planning | 显式规划 | 先生成可审计的步骤清单，再按序执行 | Plan & Execute | task_decomposer ✅ |
| 6 | PEV | 验证驱动重规划 | 每步强制 verifier，失败回重规划 | Plan-Execute-Verify | auto-loop verify ⚠️ |
| 7 | Multi-Agent | 角色分解 | 研究员/写手/审阅者拆开，流水线串接 | Multi-Agent | team-engine ✅ |
| 8 | Blackboard | 共享状态黑板 | 中间产物写共享黑板，controller 动态调度 | Blackboard | — (部分) |
| 9 | Meta-Controller | 入口路由 | 先分类再路由到专家子 agent | Meta-Controller | tier_routing ✅ |
| 10 | Ensemble | 并行冗余 | 多 agent 独立处理，aggregator 融合投票 | Ensemble | — (部分) |
| 11 | Long-term Memory | 记忆持久化 | episodic (向量) + semantic (图/KV) | Episodic + Semantic | memory_layers ✅ |
| 12 | ToT | 搜索推理 | 树形展开多条思路，边展开边打分剪枝 | Tree of Thought | ❌ 缺失 |
| 13 | Mental Loop | 行动前模拟 | 真正执行前先在内部世界模型预演 | Counterfactual | ❌ 缺失 |
| 14 | Dry-Run | 副作用闸门 | 有副作用的操作先 dry-run + 审核 | Side-effect Gating | ❌ 缺失 |
| 15 | Metacognitive | 自我边界建模 | 知道自己擅长什么、不擅长什么 | Self-boundary | ❌ 缺失 |
| 16 | Self-Improve | 迭代改进循环 | editor 打分 + writer 改稿 + 样本沉淀 | Iterative Refinement | auto-loop ⚠️ |
| 17 | Cellular Automata | 涌现计算 | 无中心 LLM，局部规则涌现全局行为 | Emergence | — (研究) |

## 架构演化核心洞察

### 控制能力递增
每一代不是替换上一代，而是在上一代基础上**增加一种控制能力**：
- Reflection 增加了质量控制
- Tool Use 增加了世界交互
- ReAct 增加了持续决策循环
- Planning 增加了显式流程控制
- PEV 增加了验证回路
- Multi-Agent 增加了角色分工
- Side-effect Gating 增加了副作用隔离
- Self-boundary 增加了自我认知

### 三个关键断崖
1. **文本→结构化跨越**（Tool Use）：序列化/反序列化是第一道硬边界
2. **线性→循环拓扑**（ReAct）：从"单次调用"到"持续交互系统"
3. **功能→信任**（Dry-Run + Metacognitive）：从"能做事"到"可信任"

### 失败模式递进
| 架构 | 典型失败模式 |
|------|------------|
| Reflection | 不能验证 refiner 是否真的修好了 critique 指出的问题 |
| Tool Use | 工具名幻觉、参数类型错误、返回格式不对、结果被错误综合 |
| ReAct | 局部贪心：每次只基于当前 observation 决策，容易走弯路 |
| Planning | 计划可能过时：环境变化后仍按原计划执行 |
| PEV | 验证器本身可能有误判：false positive 导致无限重试 |
| Multi-Agent | 角色边界模糊、通信开销爆炸、死锁 |
| Side-effect Gating | 过度保守：dry-run 通过但真实环境有差异 |

## AgentGuard 架构覆盖度评估

**已覆盖（8/17）**: Reflection, Tool Use, ReAct, Planning, Multi-Agent, Meta-Controller, Long-term Memory, 部分 PEV

**需补充（3/17）**: Side-effect Gating, Self-boundary Reasoning, ToT

**不适用（6/17）**: Blackboard (可选), Ensemble (可选), Mental Loop (研究), Self-Improve (部分已有), Cellular Automata (研究)

## 参考实现

- 项目: all-agentic-architectures (GitHub)
- 框架: agno (Python)
- 17 个 Jupyter Notebook，每种架构一个独立实现
