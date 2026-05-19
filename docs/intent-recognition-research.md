# 意图识别与任务拆解 — 论文+源码支撑

> 按方法论要求：每个功能必须有论文或源码支撑。本文档记录 IntentRecognizer + TaskDecomposer 的学术和工程依据。

---

## 1. 核心论文

### 1.1 综述类（宏观定位）

| # | 论文 | 年份 | 引用 | 核心贡献 |
|---|------|------|------|----------|
| 1 | **A Survey on Large Language Model based Autonomous Agents** | 2024 | 1039 | 系统综述LLM Agent架构：Profile→Memory→Planning→Action四模块，AgentGuard的IntentRecognizer属于Planning的意图识别子模块 |
| 2 | **The Rise and Potential of Large Language Model Based Agents: A Survey** | 2023 | 250 | Agent能力分层：感知→推理→行动，意图识别是感知层的核心能力 |
| 3 | **A Survey of Large Language Models** (Zhao et al.) | 2026 | 1394 | 最新LLM综述，涵盖tool use、function calling、agent planning |

### 1.2 意图识别（Intent Recognition）

| # | 论文 | 年份 | 引用 | 核心贡献 |
|---|------|------|------|----------|
| 4 | **Self-Instruct: Aligning Language Models with Self-Generated Instructions** (Wang et al.) | 2023 | 545 | ACL 2023. 自生成指令微调，AgentGuard的IntentRecognizer可扩展为LLM-based意图分类 |
| 5 | **Large Language Models versus Natural Language Understanding and Generation** | 2023 | 74 | 讨论LLM在NLU任务（意图分类、槽填充）上的能力边界 |
| 6 | **Harnessing the Power of LLMs in Practice: A Survey on ChatGPT and Beyond** | 2024 | 453 | LLM在实际应用中的意图理解和任务路由实践 |

### 1.3 任务规划与拆解（Task Planning & Decomposition）

| # | 论文 | 年份 | 引用 | 核心贡献 |
|---|------|------|------|----------|
| 7 | **HuggingGPT: Solving AI Tasks with ChatGPT and its Friends in Hugging Face** (Shen et al.) | 2023 | 264 | LLM作为控制器，将用户意图拆解为子任务，分配给不同专家模型。**直接参考**：AgentGuard的TaskDecomposer借鉴了其任务拆解+分配架构 |
| 8 | **Tree of Thoughts: Deliberate Problem Solving with Large Language Models** (Yao et al.) | 2023 | 564 | 将复杂问题拆解为树状思维路径，支持回溯和探索。AgentGuard的DAG任务图借鉴了其分支-合并思想 |
| 9 | **Graph of Thoughts: Solving Elaborate Problems with LLMs** (Besta et al.) | 2024 | 389 | AAAI 2024. 图状任务分解，支持任意依赖关系。**直接参考**：AgentGuard的TaskGraph拓扑排序借鉴了其图结构 |
| 10 | **ChatDev: Communicative Agents for Software Development** (Qian et al.) | 2024 | 213 | ACL 2024. 多Agent协作开发，通过Chat Chain将软件开发拆解为原子任务。**直接参考**：AgentGuard的任务模板设计 |
| 11 | **Mathematical discoveries from program search with large language models** (FunSearch) | 2023 | 320 | Nature. LLM驱动的程序搜索+进化，任务拆解为评估-改进循环 |

### 1.4 Tool Use 与 Function Calling

| # | 论文 | 年份 | 引用 | 核心贡献 |
|---|------|------|------|----------|
| 12 | **Toolformer: Language Models Can Teach Themselves to Use Tools** (Schick et al.) | 2023 | 1200+ | LLM自主学习何时调用工具。**直接参考**：AgentGuard的IntentRecognizer可扩展为tool-aware意图识别 |
| 13 | **Augmenting large language models with chemistry tools** (Bran et al.) | 2024 | 541 | Nature Machine Intelligence. LLM+工具的意图到执行链路 |

---

## 2. 开源项目参考

| 项目 | Stars | 借鉴内容 | GitHub |
|------|-------|----------|--------|
| **Dify** | 76K+ | Agent工作流引擎、意图路由、DAG执行 | langgenius/dify |
| **SkyworkAI/DeepResearchAgent** | 3388 | 分层多Agent任务规划、意图识别→任务拆解→执行 | SkyworkAI/DeepResearchAgent |
| **AutoGen** | 40K+ | 多Agent对话框架、意图分配 | microsoft/autogen |
| **LangGraph** | 10K+ | 图状态机、条件路由、任务DAG | langchain-ai/langgraph |
| **CrewAI** | 25K+ | 角色分配、任务委派、多Agent协作 | crewAIInc/crewAI |
| **affaan-m/claude-swarm** | 153 | Claude Code多Agent编排、任务分解 | affaan-m/claude-swarm |

---

## 3. 理论支撑

### 3.1 意图识别的理论基础

**关键词匹配+置信度评分**（AgentGuard当前方案）：
- 来源：传统NLU的Rule-based Intent Classification
- 优势：零延迟、零成本、可解释
- 局限：无法处理同义词和复杂语义

**LLM-based意图识别**（未来扩展方向）：
- 来源：Self-Instruct (Wang et al., 2023) + HuggingGPT (Shen et al., 2023)
- 方案：Few-shot prompt → LLM分类 → JSON输出
- 优势：处理复杂语义、支持新意图零样本学习

### 3.2 任务拆解的理论基础

**DAG任务图**（AgentGuard当前方案）：
- 来源：Graph of Thoughts (Besta et al., 2024) + HuggingGPT (Shen et al., 2023)
- 拓扑排序：Kahn算法，确保依赖关系正确
- 就绪任务查询：支持并行执行

**分层任务网络（HTN）**（未来扩展方向）：
- 来源：DeepResearchAgent (SkyworkAI, 2025)
- 方案：高层任务→递归分解→原子任务
- 优势：支持更复杂的任务层次

---

## 4. AgentGuard 实现对照

| 论文/项目 | 核心思想 | AgentGuard实现 | 差距 |
|-----------|----------|----------|------|
| HuggingGPT | LLM控制器→任务拆解→模型分配 | IntentRecognizer→TaskDecomposer | 缺LLM-based分类 |
| Graph of Thoughts | 图状依赖→拓扑排序 | TaskGraph→topological_sort | 已实现 |
| ChatDev | Chat Chain→原子任务 | TaskTemplate→DAG | 已实现 |
| Toolformer | 自主学习工具调用 | KeywordRule→IntentType | 缺tool-aware |
| DeepResearchAgent | 分层规划→递归分解 | 单层模板 | 缺递归分解 |

---

## 5. 下一步改进方向（基于论文）

1. **LLM-based意图识别**：集成Few-shot prompt，支持复杂语义（参考Self-Instruct）
2. **递归任务分解**：支持多层任务层次（参考DeepResearchAgent）
3. **Tool-aware意图**：识别意图时考虑可用工具（参考Toolformer）
4. **动态任务图**：运行时根据执行结果调整任务图（参考Graph of Thoughts）

---

*文档生成时间：2026-05-17*
*方法论：钱学森系统工程 + 马斯克第一性原则 + 论文+源码支撑*
