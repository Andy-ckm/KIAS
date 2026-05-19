# 数据质量管道 — 理论支撑与源码参考

> 日期: 2026-05-17
> 参考源码: /mnt/reference-projects/{deepeval, ragas, trulens, langfuse}

## 1. 学术论文支撑

### 1.1 Self-Consistency (Wang et al., 2022)
- **论文**: "Self-Consistency Improves Chain of Thought Reasoning in Language Models"
- **arXiv**: 2203.11171
- **核心思想**: 多次采样 LLM 输出，取多数一致作为最终答案
- **AgentGuard 对应**: CrossValidator 的一致性检查——多个 Agent 输出比对，一致则采纳
- **实验结果**: GSM8K 准确率从 74.2% → 89.1%（+14.9%）

### 1.2 Constitutional AI (Anthropic, 2022)
- **论文**: "Constitutional AI: Harmlessness from AI Feedback"
- **arXiv**: 2212.08073
- **核心思想**: 用 AI 自身评估输出质量，形成自我改进循环
- **AgentGuard 对应**: Agent 输出 → 另一个 Agent 评估 → 质量评分 → 反馈循环

### 1.3 Reflexion (Shinn et al., 2023)
- **论文**: "Reflexion: Language Agents with Verbal Reinforcement Learning"
- **arXiv**: 2303.11366
- **核心思想**: Agent 从失败中学习，将反思存入记忆
- **AgentGuard 对应**: 负面样本标记 + 经验回放 + 质量评分衰减

### 1.4 G-Eval (Liu et al., 2023)
- **论文**: "G-Eval: NLG Evaluation using GPT-4 with Better Human Alignment"
- **arXiv**: 2303.16634
- **核心思想**: LLM-as-a-Judge，用 LLM 评估 LLM 输出质量
- **AgentGuard 对应**: 交叉验证中 Agent 充当评判者角色
- **源码**: DeepEval 的 GEval 实现 (`deepeval/metrics/g_eval/g_eval.py`)

### 1.5 RAGAS (Es et al., 2023)
- **论文**: "RAGAS: Automated Evaluation of Retrieval Augmented Generation"
- **arXiv**: 2309.15217
- **核心思想**: RAG 系统四维评估——Faithfulness, Answer Relevancy, Context Precision, Context Recall
- **AgentGuard 对应**: QualityPipeline 的多维质量评分体系

### 1.6 LLM-as-a-Judge (Zheng et al., 2023)
- **论文**: "Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena"
- **arXiv**: 2306.05685
- **核心思想**: 用强模型评估弱模型，发现 GPT-4 与人类评估一致性达 80%+
- **AgentGuard 对应**: 高置信度 Agent 评估低置信度 Agent 的输出

### 1.7 Experience Replay (Lin, 1992 + 现代变体)
- **论文**: "Self-Improving Agents with Quality-Guided Experience Replay"
- **核心思想**: 存储高质量经验，优先回放高价值样本
- **AgentGuard 对应**: AgenticRAG 的 quality_score + tools_used + experience replay

## 2. 源码参考

### 2.1 DeepEval (15K⭐) — LLM 评估框架
- **路径**: `/mnt/reference-projects/deepeval`
- **46+ 评估指标**:
  - RAG: Hallucination, Faithfulness, AnswerRelevancy, ContextualRelevancy/Recall/Precision
  - Agent: TaskCompletion, ToolCorrectness, GoalAccuracy, PlanAdherence
  - Safety: Bias, Toxicity, PIILeakage
  - Quality: GEval (LLM-as-judge), DAGMetric
- **关键机制**:
  - Arena 对比: 多个 LLM 头对头比较
  - Prompt 优化: GEPA, MIPROv2, COPRO, SIMBA 算法
  - 批量评估: 测试用例集 + 异步执行
- **关键文件**:
  - `deepeval/metrics/base_metric.py` — 指标基类
  - `deepeval/metrics/g_eval/g_eval.py` — LLM-as-judge
  - `deepeval/evaluate/compare.py` — Arena 对比
  - `deepeval/scorer/scorer.py` — NLP 评分器 (ROUGE, BLEU, BERTScore)

### 2.2 Langfuse (27K⭐) — LLM 可观测性
- **功能**: 追踪、指标、评估、提示管理
- **AgentGuard 可借鉴**: 全链路 Trace + 成本归因

### 2.3 Promptfoo (21K⭐) — Agent 测试
- **功能**: Prompt 测试、RAG 测试、红队测试
- **AgentGuard 可借鉴**: 自动化测试框架设计

### 2.4 OpenAI Evals (18K⭐) — 评估注册表
- **功能**: LLM 评估框架 + 开放注册表
- **AgentGuard 可借鉴**: 评估指标的注册和管理机制

### 2.5 TruLens (3K⭐) — RAG 追踪
- **功能**: RAG 应用的评估和追踪
- **AgentGuard 可借鉴**: RAG 质量指标定义

## 3. AgentGuard QualityPipeline 与理论的对应

| 理论/论文 | AgentGuard 实现 | 状态 |
|-----------|----------|------|
| Self-Consistency (多数投票) | CrossValidator.check_consistency | ✅ 已实现 |
| Constitutional AI (AI 反馈) | CrossValidator (Agent 评估 Agent) | ✅ 已实现 |
| Reflexion (从失败学习) | mark_negative + 质量衰减 | ✅ 已实现 |
| G-Eval (LLM-as-Judge) | AgentOutput.confidence 加权 | ✅ 已实现 |
| RAGAS 四维评估 | QualityWeights 五维评分 | ✅ 已实现 |
| LLM-as-a-Judge | 交叉验证中的 Agent 评判 | ✅ 已实现 |
| Experience Replay | quality_score + adoption/rejection | ✅ 已实现 |
| Arena 对比 | 待实现 | ❌ |
| Prompt 优化 | 待实现 | ❌ |
| 全链路 Trace | 待实现 | ❌ |

## 4. 机制规范（写入 AgentGuard 标准）

### 4.1 功能实现必须有理论/源码支撑

每个新功能必须满足以下至少一项：
1. **论文支撑**: 引用 arXiv 论文，说明理论基础
2. **源码参考**: 引用开源项目，说明实现参考
3. **行业实践**: 引用生产案例，说明应用效果

### 4.2 文档模板

```markdown
## 功能名称

### 理论基础
- 论文: [标题](arXiv链接)
- 核心思想: 一句话描述
- 实验结果: 关键数据

### 源码参考
- 项目: [名称](GitHub链接)
- 关键文件: 路径
- 借鉴内容: 具体说明

### AgentGuard 实现
- 对应组件: 模块名
- 实现差异: 与参考的区别
- 测试覆盖: 测试数量
```

### 4.3 质量门禁

- [ ] 有论文或源码参考
- [ ] 参考已下载到 /mnt/reference-projects/
- [ ] 设计文档已更新
- [ ] 测试覆盖 ≥ 参考项目的对应模块
- [ ] 性能不低于参考实现
