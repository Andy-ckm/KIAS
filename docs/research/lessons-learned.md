# AgentGuard 从开源项目学到的教训与经验

> 最后更新: 2026-05-18
> 来源: GBrain、Harness Engineering、飞书 CLI、all-agentic-architectures (17种架构)
> 状态: 持续更新

## 一、核心教训（按优先级排序）

### 1. 测试通过 ≠ 功能生效
**来源**: AgentGuard 自身审计发现
**教训**: 写模块 → 写测试 → 测试通过 → 就以为完了。但没有接入主循环的模块是死代码。
**行动**: 每个新模块必须有明确的"接入点"（哪个函数调用它），接入后必须验证调用链。

### 2. 控制流设计 > Prompt Engineering
**来源**: all-agentic-architectures
**教训**: Agent 架构的本质不是 prompt，而是控制流。状态有没有被正确建模、控制流有没有被显式表达、错误能不能被局部截断、副作用能不能被关进闸门、系统知不知道自己什么时候该停。
**行动**: AgentGuard 的每个模块都必须回答六个问题：解决什么问题、State是什么、拓扑是什么、Router怎么工作、失败模式是什么、什么时候升级。

### 3. 每一代架构增加一种控制能力
**来源**: all-agentic-architectures
**教训**: Reflection增加质量控制，Tool Use增加世界交互，ReAct增加持续决策循环，Planning增加显式流程控制，PEV增加验证回路，Multi-Agent增加角色分工，Side-effect Gating增加副作用隔离，Self-boundary增加自我认知。
**行动**: 不要试图一步到位。按演化路径逐步添加控制能力。

### 4. 灵魂在 Harness，不在 Model
**来源**: Harness Engineering
**教训**: Agent = Model + Harness。模型是可替换的执行器，Harness（skills + memory + approval + audit + knowledge）才是 Agent 的身份和能力。
**行动**: AgentGuard 的差异化在 Harness 层，不是模型层。model-router 解决模型选择，但 skills/memory/audit 才是核心价值。

### 5. Markdown + Git 是人类与 AI 共享的真值源
**来源**: GBrain
**教训**: 用 Markdown 文件存储实体知识，Git 做版本控制。人类和 AI 共享同一份真值源。数据库崩了从 Git 重建。
**行动**: AgentGuard 的知识层应该以 Markdown 为真值源，数据库为索引层。

### 6. 零 LLM 调用的知识图谱
**来源**: GBrain
**教训**: 正则 + 字符串匹配提取实体关系（works_at/invested_in/founded），成本为零，图谱查询比语义猜测准。
**行动**: entity_extractor 已实现，必须接入 knowledge ingest 流程。

### 7. 夜间巩固循环（Dream Cycle）
**来源**: GBrain
**教训**: 白天收集信号，晚上 Minions 跑确定性任务（去重、充实、重建索引），0 token 成本。
**行动**: AgentGuard 的 auto-loop 应该区分"需要 LLM 的任务"和"确定性任务"，后者用代码直接跑。

### 8. 连接层是落地瓶颈
**来源**: 飞书 CLI
**教训**: 模型不是瓶颈，连接才是。AgentGuard 的 MCP 模块已完成，但工具连接层（飞书/钉钉/企微/Slack）还没做。
**行动**: 飞书 CLI 接入作为第一个企业级集成场景。

### 9. 三层 CLI 架构验证了 Skills 设计
**来源**: 飞书 CLI
**教训**: Shortcut → API Command → Raw API 三层。AgentGuard 的 Skills 系统是同一模式。
**行动**: Skills 设计方向正确，继续深化。

### 10. 验证驱动重规划（PEV）
**来源**: all-agentic-architectures
**教训**: 每步执行完强制过 verifier，失败就回到重规划。不要把验证当成事后检查。
**行动**: auto-loop 的 verify 阶段必须能触发重新 plan，不仅仅是 pass/fail。

### 11. 副作用必须关进闸门
**来源**: all-agentic-architectures #17
**教训**: 任何有副作用的操作先 dry-run + 审核，通过后才执行。
**行动**: side_effect_gate 已实现，已接入 auto-loop implement_fix。

### 12. 系统要知道自己什么时候该停
**来源**: all-agentic-architectures #18
**教训**: Self-boundary Reasoning — 知道自己擅长什么、不擅长什么，据此选择亲自做、调工具、还是交给人。在医疗/法律/金融场景，最强的能力是"拒绝"。
**行动**: self_boundary 已实现，已接入 auto-loop start_loop。

## 二、架构演化全景（17种架构 → AgentGuard 映射）

| # | 架构 | 核心能力 | AgentGuard 模块 | 状态 |
|---|------|---------|----------|------|
| 1 | Reflection | 生成+评估+修正 | quality_pipeline | ✅ 已有 |
| 2 | Tool Use | 结构化世界交互 | tool-executor + MCP | ✅ 已有 |
| 3 | ReAct | 观察-行动循环 | auto-loop | ✅ 已有 |
| 4 | Planning | 控制流对象化 | task_decomposer | ✅ 已有 |
| 5 | PEV | 验证驱动重规划 | auto-loop verify | ⚠️ 部分 |
| 6 | Multi-Agent | 认知分工 | team-engine | ✅ 已有 |
| 7 | Blackboard | 共享黑板 | — | ❌ 缺失 |
| 8 | Meta-Controller | 入口路由 | tier_routing | ✅ 已有 |
| 9 | Ensemble | 并行冗余 | — | ❌ 缺失 |
| 10 | Long-term Memory | 记忆持久化 | memory_layers | ✅ 已有 |
| 11 | ToT | 搜索推理 | — | ❌ 缺失 |
| 12 | Mental Loop | 行动前模拟 | — | ❌ 缺失 |
| 13 | Dry-Run | 副作用闸门 | side_effect_gate | ✅ 已接入 |
| 14 | Metacognitive | 自我边界 | self_boundary | ✅ 已接入 |
| 15 | Self-Improve | 迭代改进 | auto-loop learner | ⚠️ 部分 |
| 16 | Cellular Automata | 涌现计算 | — | 研究方向 |

## 三、失败模式清单（从17种架构学到的）

| 架构 | 典型失败模式 | AgentGuard 缓解措施 |
|------|------------|-------------|
| Reflection | 不能验证 refiner 是否真的修好了 | verifier 阶段 |
| Tool Use | 工具名幻觉、参数类型错误、序列化边界 | MCP 标准化协议 |
| ReAct | 局部贪心，每次只看当前 observation | task_decomposer 显式规划 |
| Planning | plan 错了全盘错 | PEV 验证回路 |
| PEV | verifier 误判导致无限重试 | max_retries 限制 |
| Multi-Agent | 角色冲突、通信开销 | team-engine 角色分离 |
| Side-effect Gating | dry-run 通过但真实环境有差异 | 审计链记录差异 |
| Metacognitive | 置信度估计不准 | 历史数据校准 |
| ToT | 组合爆炸 | 搜索深度限制 |

## 四、GBrain 三大模式吸收状态

| 模式 | 状态 | 实现 |
|------|------|------|
| Markdown 真值源 | ✅ 已吸收 | knowledge 层设计文档 |
| 零 LLM 知识图谱 | ⚠️ 已实现未接入 | entity_extractor.rs (390行) |
| Dream Cycle 夜间巩固 | ⚠️ 部分 | auto-loop learner |

## 五、Harness Engineering 公式映射

```
Agent = Model + Harness
```

| 公式要素 | AgentGuard 模块 | 状态 |
|---------|----------|------|
| 模型集 M | model-router | ✅ |
| 选模型 m* | scheduler + tier_routing | ✅ |
| 调 Harness h* | skills + memory + tools | ✅ |
| 任务分解 | workflow-engine + task_decomposer | ✅ |
| Loss 评估 | quality_pipeline + verifier | ✅ |
| Token 性价比 | model-router + token analytics | ✅ |
| Harness 参数学习 | auto-loop + learner | ✅ |

## 六、接入状态追踪

| 模块 | 行数 | 接入点 | 状态 |
|------|------|--------|------|
| side_effect_gate | 545 | auto-loop implement_fix | ✅ 已接入 |
| self_boundary | 525 | auto-loop start_loop | ✅ 已接入 |
| entity_extractor | 390 | knowledge memory.remember | ✅ 已接入 |
| entity_tier | 237 | knowledge memory.remember | ✅ 已接入 |
| knowledge/approval | 1566 | knowledge document workflow | ⚠️ 待接入 |
| gxp_audit (common) | 820 | auto-loop 审计日志 | ⚠️ 待接入 |
| gxp_auth (common) | 1286 | 认证流程 | ⚠️ 待接入 |
