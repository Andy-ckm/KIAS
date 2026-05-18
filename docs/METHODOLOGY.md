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

> "Agent = Model + Harness。Base model 很重要，但怎么把它用好，可能更重要。" —— 王云鹤

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

#### 4.3 KIAS 的 Harness 架构映射

| Harness 要素 | KIAS 模块 | 说明 |
|-------------|----------|------|
| 模型选择 | model-router + tier_routing | 任务复杂度→最优模型 |
| Prompt 优化 | skills + quality_pipeline | 技能文件 + 竞技场评估 |
| RAG/知识 | knowledge + graphrag + entity_extractor | 混合检索 + 知识图谱 |
| 记忆 | memory_layers + DreamConsolidator | 分层记忆 + 夜间巩固 |
| 安全/审计 | gxp_audit + gxp_auth + approval | GxP 合规全链路 |
| 工具调用 | tool-executor + mcp-protocol | 标准化工具协议 |
| 自我进化 | auto-loop + learner | 用 KIAS 开发 KIAS |

#### 4.4 灵魂之争的答案

> 如果 Harness 控制模型选择，甚至基于 Harness 数据增训模型，灵魂到底属于谁？

**KIAS 的答案**：灵魂在 Harness。模型是可替换的执行器，Harness（skills + memory + approval + audit + knowledge）才是 Agent 的身份和能力。更换模型不影响 Agent 的本质；更换 Harness 则改变 Agent 的行为。

#### 4.5 参考文献

1. 王云鹤, "Harness Engineering: Agent = Model + Harness", 知乎, 2026
2. OpenClaw, 开源 Agent 框架, 2026
3. Anthropic, Claude Code + Opus 迭代模式, 2025-2026
4. Pretrained Image Processing Transformer (IPT), 2020

---

## 二、四步开发法（强制流程）

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
| Step 1: 评估 | 第一性原则思考 | 本质是什么？真的需要吗？ |
| Step 2: 审视 | 检查现有功能 | 有重复吗？能扩展吗？ |
| Step 3: 方案 | 论文+源码支撑 | 有参考吗？已分析吗？ |
| Step 4: 开发 | 质量门禁 | fmt + clippy + test |

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
