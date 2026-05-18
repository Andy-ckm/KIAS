# KIAS 顶层方法论 — 完整原则体系

> **铁律：先评估，再开发。不评估不动手。**
> 基于钱学森系统工程理论 + 马斯克第一性原则。

---

## 一、核心原则体系

### 1. 钱学森系统工程原理（工程方法论）

#### 1.1 整体性原则
- 每个功能必须评估对系统整体的影响
- 不孤立开发，不重复造轮
- 先审视现有功能，再决定是否新增

#### 1.2 综合集成原则
- 多源知识融合（论文/代码/经验/最佳实践）
- 人机结合（AutonomyGate 控制 Agent 自主度）
- 从定性到定量渐进式演进

#### 1.3 反馈控制原则
- 正反馈：成功经验记录并增强（InspirationStream）
- 负反馈：失败案例记录并规避（QualityPipeline）
- 闭环：执行→验证→学习→改进

#### 1.4 层次分解原则
- 严格分层：L0(基础) → L1(数据) → L2(业务) → L3(接口)
- 单向依赖，禁止跨层
- 每层有每层的规律

#### 1.5 鲁棒性原则
- 熔断器：超时/限流/降级
- 重试策略：指数退避/幂等性
- 回退方案：主要路径失败后的备选

#### 1.6 可观测性原则
- Prometheus 指标导出
- 审计日志记录
- 健康检查端点（live/ready/deep）

#### 1.7 工程化原则
- 质量门禁：fmt + clippy + test，零容忍
- 源码依据：每个功能有论文或开源项目参考
- 文档同步：代码变更必须同步文档

---

### 2. 马斯克第一性原则（思维方法论）

#### 2.1 回归本质
> "把事情归结到最基本的真理，然后从那里开始推理。" —— Elon Musk

**在 KIAS 中的应用**：
- 不问"别人怎么做"，问"本质是什么"
- 不问"能不能做"，问"为什么不能做"
- 不问"怎么做"，问"为什么要这么做"

**示例**：
```
问题：KIAS 需要统一检索引擎吗？
第一性原则思考：
  1. 检索的本质是什么？
     → 从知识库中找到最相关的信息
  2. 现有模块能否做到？
     → AgenticRAG + GraphRAG 已覆盖
  3. 统一入口的本质是什么？
     → 根据查询特征，自动选择最优策略
  4. 现有模块能否做到？
     → 可以扩展 AgenticRAG，不需要新建模块
结论：扩展 AgenticRAG，不新建模块
```

#### 2.2 质疑一切假设
> "不要因为别人这么做了，就认为这是对的。" —— Elon Musk

**在 KIAS 中的应用**：
- 质疑"业界标准"：真的适合 KIAS 吗？
- 质疑"最佳实践"：真的最优吗？
- 质疑"必须做"：真的必须吗？

**示例**：
```
假设：必须用向量搜索
第一性原则质疑：
  1. 向量搜索的本质是什么？
     → 语义相似度计算
  2. KIAS 的场景需要语义相似吗？
     → 代码/技术文档，关键词匹配更精准
  3. 向量搜索的成本？
     → 需要 embedding 模型，计算成本高
结论：KIAS 场景下，关键词匹配可能更合适
```

#### 2.3 从物理定律出发
> "物理定律是唯一不能违反的。" —— Elon Musk

**在 KIAS 中的应用**：
- 尊重物理定律：计算有成本，存储有上限，网络有延迟
- 尊重数学规律：复杂度理论，信息论
- 尊重工程规律：没有银弹，权衡取舍

**示例**：
```
问题：能否实现实时 GraphRAG？
第一性原则思考：
  1. 图遍历的复杂度？
     → O(V+E)，V=节点数，E=边数
  2. 实时的要求？
     → <100ms
  3. KIAS 的知识库规模？
     → 预计 10K+ 节点
  4. 能否在 100ms 内完成？
     → 可能不行，需要缓存或预计算
结论：需要缓存策略，不能纯实时
```

---

### 3. 论文+源码支撑原则（验证方法论）

> **铁律：每个功能必须有论文或源码支撑。没有支撑 = 不做。**

#### 3.1 论文支撑
- 每个功能必须有学术论文作为理论基础
- 论文必须下载到本地分析（`/mnt/reference-projects/`）
- 论文必须写入 RAG 知识库

**示例**：
```
功能：AgenticRAG
论文支撑：
  - "Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks" (2020)
  - "Self-RAG: Learning to Retrieve, Generate, and Critique" (2023)
存储位置：/mnt/reference-projects/rag-papers/
RAG 注入：是
```

#### 3.2 源码支撑
- 每个功能必须有开源项目作为实现参考
- 源码必须下载到本地分析（`/mnt/reference-projects/`）
- 源码必须写入 RAG 知识库

**示例**：
```
功能：GraphRAG
源码支撑：
  - Microsoft GraphRAG (github.com/microsoft/graphrag)
  - Dify (github.com/langgenius/dify)
存储位置：/mnt/reference-projects/graphrag/
RAG 注入：是
```

#### 3.3 支撑验证清单

每个功能必须通过以下检查：

```markdown
## 论文+源码支撑检查清单

### 论文支撑
- [ ] 有学术论文作为理论基础
- [ ] 论文已下载到 /mnt/reference-projects/
- [ ] 论文已注入 RAG 知识库
- [ ] 论文核心思想已写入文档

### 源码支撑
- [ ] 有开源项目作为实现参考
- [ ] 源码已下载到 /mnt/reference-projects/
- [ ] 源码已注入 RAG 知识库
- [ ] 源码核心实现已写入文档

### 验证
- [ ] 论文/源码与功能需求匹配
- [ ] 论文/源码已充分分析
- [ ] 论文/源码已应用到实现中
```

---

### 4. Harness Engineering（Agent 架构方法论）

> "Agent = Model + Harness。Base model 很重要，但怎么把它用好，可能更重要。"

#### 4.1 核心公式

**公式 1**：给定任务 T 和模型集 M，对每个步骤选最优模型 m* 和最优 Harness 参数 h*
```
Agent(T, M) = Σᵢ [Select(m*, h*) | Step_i]
目标：min Loss / min TokenCost
```

**公式 2**：Model Parameters + Harness Parameters 联合优化
```
AGI_next = Model_Params ⊕ Harness_Params → 联合迭代优化
```

#### 4.2 为什么 Harness 不会被模型吃掉

1. **模型"七国八制"**：不同模型在不同任务上表现差异大，benchmark 与实际表现关联度低
2. **任务会"打架"**：快慢思考、超分去模糊等任务本质上冲突，无法用单一模型统一
3. **复杂任务需多模型协同**：多模态理解+生成、具身智能感知+决策+运控
4. **Harness 拥有控制权**：模型被动执行，Harness 主动选择策略和路由
5. **Harness 承载身份**：模型更新（GPT-4→5）Agent 身份不变，Harness 更新则身份演化

#### 4.3 Harness 五层架构

```
Layer 5: Self-Evolution  → auto-loop + learner + feedback
Layer 4: Safety          → approval + audit + policy
Layer 3: Knowledge       → RAG + GraphRAG + memory_layers + context_manager
Layer 2: Orchestration   → model-router + workflow-engine + team-engine + goal-engine
Layer 1: Runtime         → tool-executor + mcp-protocol + sandbox + agent-runtime
```

#### 4.4 KIAS 的 Harness 架构映射

| Harness 要素 | 理论来源 | KIAS 模块 | 状态 |
|-------------|---------|----------|------|
| 模型选择路由 | Harness 公式1 | model-router + tier_routing | ✅ |
| Prompt/技能 | Harness 4.3 | skills + quality_pipeline | ✅ |
| RAG/知识检索 | 2605.15184 (Grep) | knowledge + graphrag + entity_extractor | ✅ |
| 分层记忆 | 2605.13438 (Cognifold) | memory_layers + DreamConsolidator | ✅ |
| 安全/审计 | Harness + OpenClaw | gxp_audit + gxp_auth + approval | ✅ |
| 工具调用 | 2605.15184 | tool-executor + mcp-protocol | ✅ |
| 自我进化 | 2605.13821 (Evo) | auto-loop + learner | ⚠️ 闭环待完善 |
| 轻量化 Harness | 2605.15218 (CAX) | scheduler 优化 | ⚠️ 延迟优化待做 |
| 仿真测试 | harnesslabs/arbiter | N/A | ❌ 未实现 |
| 执行防火墙 | OpenClaw | sandbox (容器级) | ⚠️ 非系统调用级 |

#### 4.5 KIAS Harness 独特优势

1. **集群级 Harness**：不仅单 Agent，而是集群调度 + Agent Harness 融合
2. **声明式 API**：借鉴 K8S，Harness 配置即代码
3. **GxP 合规**：内置审计/审批/合规全链路，适合制药/金融高合规场景
4. **Rust 性能**：全 Rust 实现，零 GC 延迟
5. **完全可观测**：Prometheus + 审计日志 + 健康检查

#### 4.6 灵魂之争的答案

> 如果 Harness 控制模型选择，甚至基于 Harness 数据增训模型，灵魂到底属于谁？

**KIAS 的答案**：灵魂在 Harness。模型是可替换的执行器，Harness（skills + memory + approval + audit + knowledge）才是 Agent 的身份和能力。更换模型不影响 Agent 的本质；更换 Harness 则改变 Agent 的行为。

**实践证据**：
- model-router：Harness 决定用哪个模型，模型不知道自己被选中
- skills：Harness 定义 Agent 能力边界，模型只是执行器
- memory_layers：Harness 管理记忆，模型无状态
- auto-loop：Harness 驱动自我进化，模型不会自我改进

#### 4.7 参考文献

1. "Harness Engineering: Agent = Model + Harness", 知乎, 2026
2. Sen et al., "Is Grep All You Need? How Agent Harnesses Reshape Agentic Search", 2605.15184, 2026
3. Lin et al., "CAX-Agent: A Lightweight Agent Harness for Reliable APDL Automation", 2605.15218, 2026
4. Zhang & Gu, "Harnessing Agentic Evolution", 2605.13821, 2026
5. OpenClaw, Execution Firewall — Seccomp-locked Agent Sandbox, 2026
6. harnesslabs/arbiter, Multi-Agent Framework for Design/Simulation/Auditing, 2026
7. 1jehuang/jcode, Coding Agent Harness (Rust), 2026
8. Anthropic, Claude Code + Opus 迭代模式, 2025-2026
9. moosestack, Agent Harness for Analytics, 2026

→ 详见 docs/research/harness-engineering-analysis.md

---

## 二、Harness 思维（核心方法论）

> "Harness = 约束 + 参照 + 验证。让 Agent 模仿，而不是凭空创造。"

### 2.1 为什么需要 Harness？

| 问题 | 传统方式 | Harness 方式 |
|------|---------|-------------|
| Agent 输出不可控 | 自由生成，质量随机 | Harness 约束，输出可预测 |
| 开发效率低 | 从零开始写 | 模仿已有实现，快速复刻 |
| 质量保障难 | 事后测试 | 分阶段验证，边开发边验证 |
| 团队协作难 | 前后端串行 | SDD 驱动，并行开发 |

### 2.2 Harness 的三个核心要素

```
Harness = 约束 + 参照 + 验证

约束：定义 Agent 能做什么、不能做什么
参照：提供已有实现作为模仿对象
验证：分阶段检查，确保符合预期
```

### 2.3 KIAS 的四层 Harness

```
┌─────────────────────────────────────────────┐
│            Layer 4: 自我进化 Harness         │
│  auto-loop + feedback + pattern_evaluator   │
├─────────────────────────────────────────────┤
│            Layer 3: 安全 Harness             │
│  gxp_audit + gxp_auth + approval            │
├─────────────────────────────────────────────┤
│            Layer 2: 知识 Harness             │
│  knowledge + graphrag + entity_extractor    │
├─────────────────────────────────────────────┤
│            Layer 1: 执行 Harness             │
│  skills + team-engine + tool-executor       │
└─────────────────────────────────────────────┘
```

### 2.4 Harness 在开发中的应用

| 开发阶段 | Harness 应用 | KIAS 模块 |
|---------|-------------|----------|
| 需求分析 | 约束：定义功能边界 | skills |
| 设计阶段 | 参照：提供已有实现 | knowledge |
| 开发阶段 | 验证：分阶段检查 | side_effect_gate |
| 测试阶段 | 验证：端到端测试 | approval + gxp_audit |

### 2.5 Harness 思维的商业价值

| 价值 | 描述 |
|------|------|
| 效率提升 | 开发效率提升 3-5 倍 |
| 质量保障 | 端到端的质量验证 |
| 合规保障 | 受监管行业敢用 |
| 成本降低 | 减少返工和调试时间 |

→ 详见 docs/business/kias-commercialization.md

---

## 三、四步开发法（强制流程）

```
┌─────────────────────────────────────────────────────┐
│  Step 1: 评估      Step 2: 审视                      │
│  ─────────────    ─────────────                      │
│  这个功能           现有系统                          │
│  真的需要吗？       能覆盖吗？                        │
│       │                  │                           │
│       ▼                  ▼                           │
│  Step 3: 方案      Step 4: 开发                      │
│  ─────────────    ─────────────                      │
│  怎么做？           按方案执行                        │
│  做到什么程度？     不超范围                          │
└─────────────────────────────────────────────────────┘

违反铁律 = 返工。不接受"先堆上去再说"。
```

**每个步骤的要求**：

| 步骤 | 要求 | 检查项 |
|------|------|--------|
|| Step 1: 评估 | 第一性原则思考 | 本质是什么？真的需要吗？ |
|| Step 2: 审视 | 检查现有功能 | 有重复吗？能扩展吗？ |
|| Step 3: 方案 | 论文+源码支撑 | 有参考吗？已分析吗？ |
|| Step 4: 开发 | 质量门禁 | fmt + clippy + test |

**企业家思维补充**：
- **马斯克第一性原则**：从物理定律出发，质疑所有假设
- **丰田五问法**：连问5个为什么，找到根本原因
- **钱学森系统工程论**：四层架构（L0-L3），单向依赖，禁止循环

---

## 三、评估清单（每个新功能/PR 必须通过）

### 第一性原则评估
- [ ] 本质是什么？
- [ ] 真的需要吗？
- [ ] 质疑了所有假设吗？
- [ ] 从物理定律出发了吗？

### 重复性评估
- [ ] 现有功能能否覆盖？
- [ ] 能否扩展现有模块？

### 论文+源码支撑
- [ ] 有论文支撑吗？
- [ ] 有源码支撑吗？
- [ ] 已下载分析吗？
- [ ] 已注入 RAG 吗？

### 整体影响评估
- [ ] 架构分层影响？
- [ ] 编译时间影响？
- [ ] 其他 crate 影响？

---

## 四、质量门禁（零容忍）

```bash
# 每次提交前必须通过
cargo fmt --all -- --check        # 格式检查
cargo clippy --workspace -- -D warnings  # 静态分析
cargo test --workspace             # 全量测试
```

**不通过 = 不提交。没有例外。**

---

## 五、架构分层规则

```
L0: common                    ← 基础类型、错误、配置
L1: data-store                ← SQLite 持久化层
L2: scheduler, controller, workflow-engine, team-engine, knowledge, ...
L3: api-server, kias-main
```

**规则**：
- 单向依赖：L3 → L2 → L1 → L0
- 禁止跨层：L2 不能依赖 L3
- 禁止循环：A → B → A

---

## 六、知识管理规则

### 知识注入
- 论文、代码、经验、最佳实践 → RAG 知识库
- 注入前必须评估：这个知识对 KIAS 有什么价值？

### 知识使用
- 开发新功能前，先搜索知识库
- 基于已有知识决策，不凭空想象

### 知识更新
- 发现过时知识，立即更新
- 保持知识库与代码同步

---

## 七、开发节奏

### Sprint 规划
1. 审视架构：检查现有功能
2. 评估需求：确定开发优先级
3. 制定方案：明确范围和验收标准
4. 执行开发：按方案实施
5. 回顾总结：检查是否遵守方法论

### 每日检查
- 今天做了什么？
- 是否遵守四步法？
- 是否有功能重复？
- 是否有论文+源码支撑？

---

## 八、违规处理

| 违规行为 | 处理方式 |
|---------|---------|
| 跳过评估直接开发 | 返工，重新评估 |
| 功能重复 | 删除重复部分，扩展现有模块 |
| 质量门禁不通过 | 修复后重新提交 |
| 文档不同步 | 补充文档 |
| 无论文+源码支撑 | 停止开发，补充支撑 |

---

## 九、参考文献

### 钱学森系统工程理论
1. 钱学森, 《论系统工程》, 1982
2. 钱学森, 《创建系统学》, 2001
3. 钱学森, 于景元, 戴汝为, "一个科学新领域——开放的复杂巨系统及其方法论", 1990

### 马斯克第一性原则
1. Elon Musk, "The Henry Ford of Rockets", interview, 2003
2. Elon Musk, "I think it's important to reason from first principles rather than by analogy", TED Talk, 2013
3. Ashlee Vance, "Elon Musk: Tesla, SpaceX, and the Quest for a Fantastic Future", 2015

### 论文+源码支撑
1. "Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks", 2020
2. "Self-RAG: Learning to Retrieve, Generate, and Critique", 2023
3. Microsoft GraphRAG, 2024
4. Dify, 2023

---

**本方法论是 KIAS 开发的强制执行原则。违反本方法论的 PR 不予合并。**

*最后更新：2026-05-17*

## 5. 防死代码铁律（2026-05-18 新增）

> **写了没接入 = 没写。测试通过 ≠ 功能生效。**

### 5.1 写模块之前的三个必答问题

在写任何新模块之前，必须先回答：

1. **谁调用它？** — 具体到哪个文件的哪个函数
2. **调用链是什么？** — 从用户请求/定时任务/事件触发到这个模块的完整路径
3. **什么测试证明它接入了？** — 不是模块内部的单元测试，是集成测试证明调用链通了

如果答不出来，不能写代码。

### 5.2 提交前检查

每次 `git commit` 之前，对新增的 `pub fn` / `pub struct` 执行：

```bash
# 检查新模块是否被非测试代码引用
grep -rn "新模块名" --include="*.rs" . | grep -v "target/" | grep -v "#[cfg(test)]" | grep -v "pub mod"
```

如果结果为空 → 死代码，不能提交。

### 5.3 审计流程

每周执行一次全面审计：

```bash
# 找出所有只在 mod 声明和测试中出现的模块
for mod in $(grep "pub mod" crates/*/src/lib.rs | awk '{print $3}' | tr -d ';'); do
  refs=$(grep -rn "$mod" --include="*.rs" . | grep -v "target/" | grep -v "pub mod" | grep -v "#[cfg(test)]" | wc -l)
  if [ "$refs" -eq 0 ]; then
    echo "DEAD CODE: $mod"
  fi
done
```

### 5.4 因果链

```
写模块 → 写测试 → 测试通过 → 以为完了
                                    ↓
                            没接入主循环 = 死代码
                                    ↓
                            用户投诉"功能不生效"
                                    ↓
                            返工（双倍成本）
```

**正确的流程：**

```
评估谁调用 → 写接入点 → 写模块 → 写测试 → 验证调用链 → 提交
```

## 6. 灵魂 × 骨架（2026-05-18 新增）

> 来源：阿里云 CIO 蒋林泉《AI 时代产研组织效能规模化提升实践》

### 6.1 核心公式

```
灵魂（业务价值）× 骨架（核心建模）= 90%+ 的价值
```

骨架占 10%，灵魂占 90%。没有灵魂的骨架是精密的空壳。

### 6.2 三个致命教训

1. **代码首先是负债** — "增加的大量代码『可能』是资产，但『一定』是负债。"代码进入生产环境后，维护成本、系统复杂度、依赖关系管理立刻产生。

2. **AI 生码率是毒指标** — AI 生成的代码行数没有意义，因为代码行数不加权。自动生成单元测试、补充注释、胶水代码——这些价值密度最低，却是 AI 生码率最高的部分。

3. **定义清楚一个问题，这个问题就解决了 95%** — 大多数团队在"证明 AI 有用"，而不是"用 AI 解决业务问题"。

### 6.3 KIAS 的灵魂定义

**KIAS = 专为重度监管与复杂架构打造的 AI Agent 合规免疫系统**

核心问题：让高管和干系人，敢于在核心生产环境里使用 AI Agent。

目标客户：制药 / 医疗器械 / 金融等受监管行业的中型企业（200-2000 人）。

核心场景：CCR（系统变更控制）—— 变更请求 → 审批流 → 副作用预演 → 电子签名 → 审计报告。

### 6.4 效果 × 效率

- **Effectiveness（做对的事）**：模块是否产生真实业务价值
- **Efficiency（把事做对）**：AI 辅助提升开发速度

先 Effectiveness，后 Efficiency。做对的事比把事做对重要 100 倍。

### 6.5 反模式清单

| 反模式 | 症状 | KIAS 教训 |
|--------|------|----------|
| 局部战术勤奋 | 代码行数很多，业务价值为零 | 5369 行死代码 |
| Vibe Coding 上生产 | 生成代码快，但不符合工程规范 | 先生成再接入=顺序反了 |
| AI 生码率崇拜 | 追踪 AI 生成代码的比例 | 应追踪"接入率"和"业务价值" |
| Agent 囤积 | 看到好架构就吸收，不问是否需要 | 17 种架构不需要全实现 |
